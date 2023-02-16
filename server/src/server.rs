#![allow(unused_imports)]

use std::{env, iter};
use std::borrow::BorrowMut;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Error, Write};
use std::net::{AddrParseError, SocketAddr};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use chrono::Local;

use futures::Future;
use futures::stream::Stream;
use redis::{Commands, Connection, RedisError, RedisResult};
use sqlparser::dialect::MySqlDialect;
use tokio::runtime::Builder;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Core;

use crate::{meta, sys_assistant_client, utils};
use crate::meta::CacheConfigEntity;
use crate::protocol::{Action, ConnectionContext, ConnReader, ConnWriter, Packet, PacketHandler, PacketType, Pipe};
use crate::protocol::packet_writer::PacketWriter;

use crate::sys_config::{ServerConfig, VirtDBConfig};
use crate::sys_assistant_client::{add_cache_task, CacheTaskInfo, ExecLog};


pub fn start(sys_config: VirtDBConfig) -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = format!("0.0.0.0:{:?}", sys_config.clone().server.port);
    let server_addr = SocketAddr::from_str(server_addr.as_str()).unwrap();

    // Create the tokio event loop that will drive this server
    let mut l = Core::new().unwrap();
    // Get a reference to the reactor event loop
    let handle = l.handle();
    // Create a TCP listener which will listen for incoming connections
    let local_server_socket = TcpListener::bind(&server_addr, &l.handle()).unwrap();
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
    let done = local_server_socket.incoming().for_each(move |(client_input_stream, _)| {
        return rt.block_on(async {
            let sys_config = sys_config.clone();
            //TODO pool mysql+redis ?
            let sys_config = sys_config.clone();

            let mysql_config = sys_config.clone().mysql;
            let mysql_ip = mysql_config.ip;
            let mysql_port = mysql_config.port;

            let redis_conn_result = redis_client.get_connection();
            if let Err(err) = redis_conn_result {
                warn!("Connect Backend Redis fail.err:{:?}",err);
                return Ok(());
            }
            let redis_conn  = redis_conn_result.unwrap();

            let mysql_addr = format!("{}:{}", mysql_ip, mysql_port);
            let mysql_addr_result = SocketAddr::from_str(mysql_addr.as_str());
            if let Err(err) = mysql_addr_result{
                warn!("Connect Backend MySQL fail.err:{:?}",err);
                return Ok(());
            }
            let mysql_addr = mysql_addr_result.unwrap();

            let future = TcpStream::connect(&mysql_addr, &handle)
                .and_then(move |mysql| Ok((client_input_stream, mysql)))
                .and_then(move |(client, server)| {
                    run(client, server, redis_conn, sys_config.clone())
                });

            // tell the tokio reactor to run the future
            handle.spawn(future.map_err(|err| {
                info!("Failed to spawn future: {:?}", err);
            }));

            Ok(())
        });
    });
    match l.run(done) {
        Ok(v) => {}
        Err(err) => {
            warn!("error:{:?}",err);
        }
    };
    Ok(())
}

pub fn run(client: TcpStream, server: TcpStream, redis_conn: Connection, sys_config: VirtDBConfig) -> Pipe<VirtDBMySQLHandler> {
    Pipe::new(Rc::new(client), Rc::new(server), VirtDBMySQLHandler {
        _redis_conn: redis_conn,
        dialect: MySqlDialect {},
        server_config: sys_config.clone(),
        context: Rc::new(RefCell::new(ConnectionContext { sql: None, should_update_cache: false, fn_start_time: Local::now(), mysql_exec_start_time: Default::default(), redis_duration: 0, from_cache: false, cache_duration: 0 })),
    })
}


pub struct VirtDBMySQLHandler {
    _redis_conn: Connection,
    // NOTE: not *actually* static, but tied to our connection's lifetime.
    dialect: MySqlDialect,
    server_config: VirtDBConfig,
    context: Rc<RefCell<ConnectionContext>>,
}

impl PacketHandler for VirtDBMySQLHandler {
    fn handle_request(&mut self, p: &Packet, ctx: &mut RefMut<ConnectionContext>) -> Action {
        // print_packet_chars(&p.bytes);
        let result_action = match p.packet_type() {
            Ok(PacketType::ComQuery) => {
                // ComQuery packets just contain a SQL string as the payload
                let slice = &p.bytes[5..];
                // convert the slice to a String object
                let sql = String::from_utf8(slice.to_vec()).expect("Invalid UTF-8");

                let mut redis_duration = 0;
                let mut from_cache = false;

                debug!("sql:{}", sql);

                let redis_key = format!("cache:{:?}", sql);
                let cache_config_entity_list = meta::get_cache_config_entity_list();

                let mysql_dialect = MySqlDialect {};
                let mut cache_config_entity_option: Option<&CacheConfigEntity> = None;
                for entity in cache_config_entity_list.into_iter() {
                    if utils::sys_sql::is_pattern_match(
                        &entity.cached_sql_parser_token,
                        sql.to_uppercase().trim(),
                        &mysql_dialect,
                    ) {
                        cache_config_entity_option = Some(entity);
                        break;
                    }
                }
                trace!("cache_config_entity_option:{:?}",cache_config_entity_option);
                if cache_config_entity_option.is_none() {
                    return Action::Forward;
                }

                let redis_get_start_time = Local::now();
                let cached_value_result: Result<String, RedisError> = self._redis_conn.get(redis_key.clone());

                redis_duration = (Local::now() - redis_get_start_time).num_milliseconds();

                if let Ok(redis_v) = cached_value_result {
                    if redis_v != "" {
                        from_cache = true;
                        trace!("[handle_request]redis_v:{:?}", redis_v);
                        ctx.redis_duration = redis_duration;
                        ctx.from_cache = from_cache;
                        // let mut response_bytes = vec![];
                        let chars = redis_v.chars().into_iter();
                        let mut ps = vec![];
                        for x in chars.into_iter() {
                            ps.push(Packet::new(vec![x as u8]));
                            // response_bytes.push(x as u8);
                        }
                        return Action::Respond(ps);
                    }
                }
                // info!("sql:{:?},should_update_cache:{:?}",sql,true);
                match cache_config_entity_option{
                    None => {}
                    Some(cache_config_entity) => {
                        ctx.should_update_cache = true;
                        ctx.cache_duration = cache_config_entity.duration;
                    }
                };
                return Action::Forward;
            }
            _ => Action::Forward,
        };
        let mysql_exec_start_time = Local::now();
        ctx.mysql_exec_start_time = mysql_exec_start_time;
        result_action
    }
    fn handle_response(&mut self, p: &Packet, ctx: &mut RefMut<ConnectionContext>) -> Action {
        Action::Forward
    }

    fn handle_response_finish(&mut self, packets: Vec<Packet>, ctx: &mut RefMut<ConnectionContext>) {
        let should_update_cache = ctx.should_update_cache;
        let sql_option = ctx.sql.clone();
        if sql_option.is_none() {
            return;
        }
        let sql = sql_option.clone().unwrap();
        // info!("sql:{:?},should_update_cache:{:?}",sql,should_update_cache);
        let mysql_duration = (Local::now() - ctx.mysql_exec_start_time).num_milliseconds();
        if should_update_cache {
            let sql = sql_option.clone().unwrap();
            // println!("sql:{:?}", sql);

            // print_packet_chars(&*packet.bytes);
            let mut chars = vec![];
            for packet in packets {
                let bytes = &*packet.bytes.to_vec();
                for i in 0..bytes.len() {
                    chars.push(bytes[i] as char);
                };
            }
            let cache_v: String = chars.iter()
                .collect();
            add_cache_task(CacheTaskInfo::new(sql, cache_v, ctx.cache_duration));
        }
        let total_duration = (Local::now() - ctx.fn_start_time).num_milliseconds();
        sys_assistant_client::record_exec_log(ExecLog {
            sql_str: sql_option.unwrap(),
            total_duration,
            mysql_duration,
            redis_duration: ctx.redis_duration,
            from_cache: ctx.from_cache,
        });
        self.context = Rc::new(RefCell::new(ConnectionContext{
            sql: None,
            should_update_cache:false,
            fn_start_time: Default::default(),
            mysql_exec_start_time: Default::default(),
            redis_duration: 0,
            from_cache: false,
            cache_duration: 0,
        }));
    }

    fn get_context(&mut self) -> Rc<RefCell<ConnectionContext>> {
        self.context.clone()
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
