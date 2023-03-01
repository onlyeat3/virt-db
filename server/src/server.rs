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
use redis::{Client, Commands, Connection, RedisError, RedisResult};
use redis::cluster::{ClusterClient, ClusterConnection};
use sqlparser::dialect::MySqlDialect;
use tokio::net::TcpListener;
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

use crate::{meta, sys_assistant_client, utils};
use crate::meta::CacheConfigEntity;
use crate::protocol::{Action, ConnectionContext, ConnReader, ConnWriter, Packet, PacketHandler, PacketType, Pipe};
use crate::protocol::packet_writer::PacketWriter;
use crate::serve::{handle_client, VirtDBConnectionHandler};
use crate::sys_assistant_client::{CacheTaskInfo, ExecLog};

use crate::sys_config::{ServerConfig, VirtDBConfig};
// use crate::sys_assistant_client::{add_cache_task, CacheTaskInfo, ExecLog};
use crate::sys_redis::SysRedisClient;
use crate::utils::sys_sql::sql_to_pattern;


pub async fn start(sys_config: VirtDBConfig, exec_log_channel_sender: Sender<ExecLog>, cache_load_task_channel_sender: Sender<CacheTaskInfo>) -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = format!("0.0.0.0:{:?}", sys_config.clone().server.port);
    let mysql_addr_str = format!("{}:{}", sys_config.mysql.ip, sys_config.mysql.port);

    let listener = TcpListener::bind(server_addr.clone()).await?;
    info!("Listening on: {}", server_addr);

    let redis_config = sys_config.redis.clone();
    let redis_client = Client::open(redis_config.nodes)?;
    loop {
        let mysql_addr_str = mysql_addr_str.clone();
        let sys_config = sys_config.clone();
        let exec_log_channel_sender = exec_log_channel_sender.clone();
        let cache_load_task_channel_sender = cache_load_task_channel_sender.clone();
        let redis_client = redis_client.clone();
        let (client_stream, client_addr) = listener.accept().await?;

        info!("Accepted connection from {}", client_addr);
        tokio::spawn(async move {
            let mut redis_conn = redis_client.get_async_connection().await.unwrap();
            let conn_handler = VirtDBConnectionHandler::new(redis_conn, sys_config, exec_log_channel_sender, cache_load_task_channel_sender);
            handle_client(client_stream, mysql_addr_str.clone().parse().unwrap(), conn_handler).await;
        });
    }
}
//
// pub fn run(client: TcpStream, server: TcpStream, _sys_redis_client: SysRedisClient, sys_config: VirtDBConfig, exec_log_channel_sender: Sender<ExecLog>, cache_load_task_channel_sender: Sender<CacheTaskInfo>) -> Pipe<VirtDBMySQLHandler> {
//     Pipe::new(Rc::new(client), Rc::new(server), VirtDBMySQLHandler {
//         _sys_redis_client,
//         dialect: MySqlDialect {},
//         server_config: sys_config.clone(),
//         exec_log_channel_sender,
//         cache_load_task_channel_sender,
//         context: Rc::new(RefCell::new(ConnectionContext { sql: None, should_update_cache: false, fn_start_time: Local::now(), mysql_exec_start_time: Default::default(), redis_duration: 0, from_cache: false, cache_duration: 0 })),
//     })
// }
//
//
// pub struct VirtDBMySQLHandler {
//     _sys_redis_client: SysRedisClient,
//     // NOTE: not *actually* static, but tied to our connection's lifetime.
//     dialect: MySqlDialect,
//     server_config: VirtDBConfig,
//     context: Rc<RefCell<ConnectionContext>>,
//     pub exec_log_channel_sender: Sender<ExecLog>,
//     pub cache_load_task_channel_sender: Sender<CacheTaskInfo>,
// }
//
// impl PacketHandler for VirtDBMySQLHandler {
//     fn handle_request(&mut self, p: &Packet, ctx: &mut RefMut<ConnectionContext>) -> Action {
//         // print_packet_chars(&p.bytes);
//         let result_action = match p.packet_type() {
//             Ok(PacketType::ComQuery) => {
//                 // ComQuery packets just contain a SQL string as the payload
//                 let slice = &p.bytes[5..];
//                 // convert the slice to a String object
//                 let sql = String::from_utf8(slice.to_vec()).expect("Invalid UTF-8");
//                 if !sql.clone().to_uppercase().starts_with("SELECT") {
//                     return Action::Forward;
//                 }
//
//                 let mut redis_duration = 0;
//                 let mut from_cache = false;
//
//                 debug!("sql:{}", sql);
//
//                 let redis_key = format!("cache:{:?}", sql);
//                 let cache_config_entity_list = meta::get_cache_config_entity_list();
//
//                 let mysql_dialect = MySqlDialect {};
//                 let mut cache_config_entity_option: Option<&CacheConfigEntity> = None;
//                 for entity in cache_config_entity_list.into_iter() {
//                     if utils::sys_sql::is_pattern_match(
//                         &entity.cached_sql_parser_token,
//                         sql.to_uppercase().trim(),
//                         &mysql_dialect,
//                     ) {
//                         cache_config_entity_option = Some(entity);
//                         break;
//                     }
//                 }
//                 trace!("cache_config_entity_option:{:?}",cache_config_entity_option);
//                 if cache_config_entity_option.is_none() {
//                     return Action::Forward;
//                 }
//
//                 let redis_get_start_time = Local::now();
//                 let cache_key_exists_check_result: RedisResult<bool> = self._sys_redis_client.exists(redis_key.clone().as_str());
//                 if let Ok(cache_key_exists) = cache_key_exists_check_result {
//                     if cache_key_exists {
//                         let cached_value_result: Result<String, RedisError> = self._sys_redis_client.get(redis_key.clone().as_str());
//
//                         redis_duration = (Local::now() - redis_get_start_time).num_milliseconds();
//
//                         if let Ok(redis_v) = cached_value_result {
//                             if redis_v != "" {
//                                 from_cache = true;
//                                 trace!("[handle_request]redis_v:{:?}", redis_v);
//                                 ctx.redis_duration = redis_duration;
//                                 ctx.from_cache = from_cache;
//                                 // let mut response_bytes = vec![];
//                                 let chars = redis_v.chars().into_iter();
//                                 let mut ps = vec![];
//                                 for x in chars.into_iter() {
//                                     ps.push(Packet::new(vec![x as u8]));
//                                     // response_bytes.push(x as u8);
//                                 }
//                                 return Action::Respond(ps);
//                             }
//                         }
//                     }
//                 }
//                 // info!("sql:{:?},should_update_cache:{:?}",sql,true);
//                 match cache_config_entity_option {
//                     None => {}
//                     Some(cache_config_entity) => {
//                         ctx.should_update_cache = true;
//                         ctx.cache_duration = cache_config_entity.duration;
//                     }
//                 };
//                 return Action::Forward;
//             }
//             _ => Action::Forward,
//         };
//         let mysql_exec_start_time = Local::now();
//         ctx.mysql_exec_start_time = mysql_exec_start_time;
//         result_action
//     }
//     fn handle_response(&mut self, p: &Packet, ctx: &mut RefMut<ConnectionContext>) -> Action {
//         Action::Forward
//     }
//
//     fn handle_response_finish(&mut self, packets: Vec<Packet>, ctx: &mut RefMut<ConnectionContext>) {
//         let should_update_cache = ctx.should_update_cache;
//         let sql_option = ctx.sql.clone();
//         if sql_option.is_none() {
//             return;
//         }
//         let sql = sql_option.clone().unwrap();
//         // info!("sql:{:?},should_update_cache:{:?}",sql,should_update_cache);
//         let mysql_duration = (Local::now() - ctx.mysql_exec_start_time).num_milliseconds();
//         if should_update_cache {
//             // print_packet_chars(&*packet.bytes);
//             let mut chars = vec![];
//             for packet in packets {
//                 let bytes = &*packet.bytes.to_vec();
//                 for i in 0..bytes.len() {
//                     chars.push(bytes[i] as char);
//                 };
//             }
//             let cache_v: String = chars.iter()
//                 .collect();
//             let send_result = self.cache_load_task_channel_sender.send(CacheTaskInfo::new(sql.clone(), cache_v, ctx.cache_duration)).await;
//             match send_result {
//                 Ok(_) => {}
//                 Err(err) => {
//                     warn!("Send ExecLog fail.err:{:?}",err);
//                 }
//             }
//         }
//         let total_duration = (Local::now() - ctx.fn_start_time).num_milliseconds();
//         //只记录select
//         match sql_to_pattern(sql.clone().as_str()) {
//             None => {}
//             Some(sql_pattern) => {
//                 let send_result = self.exec_log_channel_sender.send(ExecLog {
//                     sql_str: sql_pattern,
//                     total_duration,
//                     mysql_duration,
//                     redis_duration: ctx.redis_duration,
//                     from_cache: ctx.from_cache,
//                 });
//                 match send_result {
//                     Ok(_) => {}
//                     Err(err) => {
//                         warn!("Send ExecLog fail.err:{:?}",err);
//                     }
//                 }
//             }
//         }
//         self.context = Rc::new(RefCell::new(ConnectionContext {
//             sql: None,
//             should_update_cache: false,
//             fn_start_time: Default::default(),
//             mysql_exec_start_time: Default::default(),
//             redis_duration: 0,
//             from_cache: false,
//             cache_duration: 0,
//         }));
//     }
//
//     fn get_context(&mut self) -> Rc<RefCell<ConnectionContext>> {
//         self.context.clone()
//     }
// }
//
//
// #[allow(dead_code)]
// pub fn print_packet_chars(buf: &[u8]) {
//     for i in 0..buf.len() {
//         print!("{}", buf[i] as char);
//     }
// }
//
// #[allow(dead_code)]
// pub fn print_packet_bytes(buf: &[u8]) {
//     print!("[");
//     for i in 0..buf.len() {
//         if i % 8 == 0 {
//             println!("");
//         }
//         print!("{:#04x} ", buf[i]);
//     }
//     println!("]");
// }
