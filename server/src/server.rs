#![allow(unused_imports)]

use std::{env, iter};
use std::cell::RefCell;
use std::net::SocketAddr;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use chrono::Local;

use futures::Future;
use futures::stream::Stream;
use mysql_async::Conn;
use redis::{Commands, Connection, RedisError};
use sqlparser::dialect::MySqlDialect;
use tokio::runtime::Builder;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Core;

use mysql_proxy_rs::{Action, ConnReader, ConnWriter, OkResponse, Packet, PacketHandler, PacketType, Pipe};
use mysql_proxy_rs::packet_writer::PacketWriter;
use mysql_proxy_rs::resultset::QueryResultWriter;
use crate::{meta, sys_metrics, utils};
use crate::meta::CacheConfigEntity;

// use crate::mysql_protocol::MySQL;
use crate::sys_config::{ServerConfig, VirtDBConfig};
use crate::sys_metrics::ExecLog;

// use opensrv_mysql::*;

pub fn start(sys_config: VirtDBConfig) -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = format!("0.0.0.0:{:?}", sys_config.clone().server.port);
    let server_addr = SocketAddr::from_str(server_addr.as_str()).unwrap();

    // Create the tokio event loop that will drive this server
    let mut l = Core::new().unwrap();
    // Get a reference to the reactor event loop
    let handle = l.handle();
    // Create a TCP listener which will listen for incoming connections
    let socket = TcpListener::bind(&server_addr, &l.handle()).unwrap();
    info!("Listening on: {}", server_addr);
    let rt = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(8)
        .build()
        .unwrap();

    let redis_config = sys_config.clone().redis;
    let redis_ip = redis_config.ip;
    let redis_port = redis_config.port;
    let redis_requirepass = redis_config.requirepass;

    let redis_url = format!("redis://{}@{}:{}", redis_requirepass, redis_ip, redis_port);
    let redis_client = redis::Client::open(redis_url.to_string()).unwrap();

    // for each incoming connection
    let done = socket.incoming().for_each(move |(socket, _)| {
        rt.block_on(async {
            let sys_config = sys_config.clone();
            //TODO pool mysql+redis ?
            let sys_config = sys_config.clone();

            let mysql_config = sys_config.clone().mysql;
            let mysql_ip = mysql_config.ip;
            let mysql_port = mysql_config.port;
            let mysql_username = mysql_config.username;
            let mysql_password = mysql_config.password;
            let mysql_url = format!(
                "mysql://{}:{}@{}:{}",
                mysql_username, mysql_password, mysql_ip, mysql_port
            );
            let mysql_conn = Conn::from_url(mysql_url.to_string()).await.unwrap();

            let redis_conn = redis_client.get_connection().unwrap();

            let mysql_addr = format!("{}:{}", mysql_ip, mysql_port);
            let mysql_addr = SocketAddr::from_str(mysql_addr.as_str()).unwrap();
            let future = TcpStream::connect(&mysql_addr, &handle)
                .and_then(move |mysql| Ok((socket, mysql)))
                .and_then(move |(client, server)| {
                    run(client, server, mysql_conn, redis_conn, sys_config.clone())
                });

            // tell the tokio reactor to run the future
            handle.spawn(future.map_err(|err| {
                info!("Failed to spawn future: {:?}", err);
            }));

            // everything is great!
            // Ok(())
        });
        Ok(())
    });
    l.run(done).unwrap();
    Ok(())
}

pub fn run(client: TcpStream, server: TcpStream, mysql_conn: Conn, redis_conn: Connection, sys_config: VirtDBConfig) -> Pipe<VirtDBMySQLHandler> {
    Pipe::new(Rc::new(client), Rc::new(server), VirtDBMySQLHandler {
        _mysql_connection: mysql_conn,
        _redis_conn: redis_conn,
        dialect: MySqlDialect {},
        server_config: sys_config.clone(),
        sqls: Rc::new(RefCell::new(vec![])),
    })
}


pub struct VirtDBMySQLHandler {
    _mysql_connection: Conn,
    _redis_conn: Connection,
    // NOTE: not *actually* static, but tied to our connection's lifetime.
    dialect: MySqlDialect,
    server_config: VirtDBConfig,
    sqls: Rc<RefCell<Vec<String>>>,
}

impl PacketHandler for VirtDBMySQLHandler {
    fn handle_request(&mut self, p: &Packet, client_reader: &ConnReader, client_writer: &ConnWriter, client_packet_writer: &mut PacketWriter) -> (Action,bool) {
        // print_packet_chars(&p.bytes);
        match p.packet_type() {
            Ok(PacketType::ComQuery) => {
                // ComQuery packets just contain a SQL string as the payload
                let slice = &p.bytes[5..];
                // convert the slice to a String object
                let sql = String::from_utf8(slice.to_vec()).expect("Invalid UTF-8");

                let sql_str = utils::normally(&self.dialect,sql.as_str());
                let mut mysql_duration = 0;
                let mut redis_duration = 0;
                let mut from_cache = false;
                let fn_start_time = Local::now();

                debug!("sql:{}", sql);
                let redis_key = format!("cache:{:?}", sql);
                let cache_config_entity_list = meta::get_cache_config_entity_list();

                let mysql_dialect = MySqlDialect {};
                let mut cache_config_entity_option: Option<&CacheConfigEntity> = None;
                for entity in cache_config_entity_list {
                    if utils::is_pattern_match(
                        &*entity.sql_template.to_uppercase().trim(),
                        sql.to_uppercase().trim(),
                        &mysql_dialect,
                    ) {
                        cache_config_entity_option = Some(entity);
                        break;
                    }
                }
                if cache_config_entity_option.is_none() {
                    return (Action::Forward,false);
                }
                let redis_get_start_time = Local::now();
                let cached_value_result: Result<String, RedisError> = self._redis_conn.get(redis_key.clone());

                redis_duration = (Local::now() - redis_get_start_time).num_milliseconds();

                if let Ok(redis_v) = cached_value_result {
                    from_cache = true;

                    trace!("redis_v:{:?}", redis_v);

                    sys_metrics::record_exec_log(ExecLog {
                        sql_str,
                        total_duration: (Local::now() - fn_start_time).num_milliseconds(),
                        mysql_duration,
                        redis_duration,
                        from_cache,
                    });
                    let response_bytes = vec![];
                    return (Action::Respond(response_bytes),false);
                }
                return (Action::Forward,true);
            }
            _ => (Action::Forward,false),
        }
    }

    fn handle_response(&mut self, packet: &Packet, sql: Option<&String>) -> Action {
        // forward all responses to the client
        println!("sql:{:?}", sql);
        print_packet_chars(&*packet.bytes);
        Action::Forward
    }

    fn get_cached_sqls(&mut self) -> Rc<RefCell<Vec<String>>> {
        self.sqls.clone()
    }
}


#[allow(dead_code)]
pub fn print_packet_chars(buf: &[u8]) {
    for i in 0..buf.len() {
        print!("{}", buf[i] as char);
    }
}

#[allow(dead_code)]
pub fn print_packet_bytes(buf: &[u8]) {
    print!("[");
    for i in 0..buf.len() {
        if i % 8 == 0 {
            println!("");
        }
        print!("{:#04x} ", buf[i]);
    }
    println!("]");
}
