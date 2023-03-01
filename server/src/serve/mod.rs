// use byteorder::{LittleEndian, ReadBytesExt as BorderReadBytesExt};
use std::net::SocketAddr;
use std::sync::Arc;
use chrono::{DateTime, Local};

use redis::aio::{Connection};
use redis::{AsyncCommands, RedisResult};
use sqlparser::dialect::MySqlDialect;
use tokio::io as async_io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener as AsyncTcpListener, TcpStream as AsyncTcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::Sender;
use crate::meta::CacheConfigEntity;
use crate::protocol::{Packet, PacketType};
use crate::sys_assistant_client::{CacheTaskInfo, ExecLog};
use crate::sys_config::VirtDBConfig;
use crate::{meta, utils};
use crate::utils::sys_sql::sql_to_pattern;

pub enum Action {
    FORWARD,
    DROP,
    RESPONSED(Vec<u8>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProxyContext {
    pub sql: Option<String>,
    pub should_update_cache: bool,
    pub fn_start_time: DateTime<Local>,
    pub mysql_exec_start_time: Option<DateTime<Local>>,
    pub redis_duration: i64,
    pub from_cache: bool,
    pub cache_duration: i32,
}

pub async fn handle_client(
    mut client_stream: AsyncTcpStream,
    remote_addr: SocketAddr,
    conn_handler: VirtDBConnectionHandler,
) {
    let mut remote_stream = match AsyncTcpStream::connect(remote_addr).await {
        Ok(stream) => stream,
        Err(e) => {
            println!("Failed to connect to {}: {}", remote_addr, e);
            return;
        }
    };

    let (mut client_reader, mut client_writer) = client_stream.split();
    let (mut remote_reader, mut remote_writer) = remote_stream.split();

    let (ctx_sender, mut ctx_receiver) = mpsc::channel(1);

    // let client_writer_semaphore = Semaphore::new(1);
    let client_writer_lock = Arc::new(Mutex::new(client_writer));
    let client_writer_lock_a = client_writer_lock.clone();

    let conn_handler = Arc::new(Mutex::new(conn_handler));
    let conn_handler_wrapper_a = conn_handler.clone();
    let conn_handler_wrapper_b = conn_handler.clone();

    let client_to_remote = async move {
        let mut buf = [0u8; 1024];
        let conn_handler_wrapper_a = conn_handler_wrapper_a;
        loop {
            // println!();
            // println!("loop");
            let read_size = client_reader.read(&mut buf).await;
            // println!("read size:{:?}", &read_size);
            match read_size {
                Ok(n) => {
                    if n == 0 {
                        return Ok(());
                    }
                    let mut ctx = ProxyContext {
                        sql: None,
                        should_update_cache: false,
                        fn_start_time: Local::now(),
                        mysql_exec_start_time: None,
                        redis_duration: 0,
                        from_cache: false,
                        cache_duration: 0,
                    };
                    let packet = Packet::new(buf.to_vec());
                    if let Err(err) = packet.packet_type() {
                        ctx_sender.send(ctx).await.unwrap();
                        remote_writer.write_all(&buf[..n]).await?;
                        continue;
                    }
                    let packet_type = packet.packet_type().unwrap();
                    let bytes = &buf[5..n];
                    let sql_result = String::from_utf8(bytes.to_vec());
                    if let Ok(sql) = sql_result {
                        ctx.sql = Some(sql.clone());
                    }
                    let mut conn_handler = conn_handler_wrapper_a.lock().await;
                    let action = conn_handler.handle_request(&mut ctx, packet_type).await;
                    let skip = match action {
                        Action::FORWARD => false,
                        Action::DROP => true,
                        Action::RESPONSED(mut bytes) => {
                            let mut client_writer = client_writer_lock_a.lock().await;
                            // println!("sql:{:?},value from cache:true", sql.clone());
                            // println!("sql:{:?},cache_v:{:X?}", sql.clone(), String::from_utf8_lossy(&*cache_v.clone()));
                            let r = client_writer.write_all(bytes.as_mut_slice()).await;
                            if let Err(err) = r {
                                println!("write to client fail.err:{:?}", err);
                            }
                            true
                        }
                    };
                    if skip {
                        continue;
                    }
                    ctx_sender.send(ctx).await.expect("send ctx fail");

                    // println!("Received from client: {:?},type:{:#?}", String::from_utf8_lossy(&*buf[..n].to_vec()), &buf[0]);
                    remote_writer.write_all(&buf[..n]).await?;
                }
                Err(e) => {
                    println!("client_to_remote:{:#?}", e);
                    return Err(e);
                }
            }
        }
    };

    let client_writer_lock_b = client_writer_lock.clone();
    let remote_to_client = async move {
        let mut buf = [0u8; 1024];
        let mut cached_buf = vec![];
        let mut cached_ctx: Option<ProxyContext> = None;
        let client_writer_lock = client_writer_lock_b;
        let conn_handler_wrapper_b = conn_handler_wrapper_b;
        loop {
            match remote_reader.read(&mut buf).await {
                Ok(n) => {
                    if n == 0 {
                        return Ok(());
                    }

                    // println!("Received from remote: {:X?}", String::from_utf8_lossy(&buf[..n]).to_string());

                    if let Ok(new_ctx) = ctx_receiver.try_recv() {
                        //清理上次保存的记录
                        // println!("recv new ctx:{:?},old:{:?}", new_ctx, cached_ctx.clone());
                        // println!("update cache for {:?}",cached_ctx);
                        if let Some(ctx) = cached_ctx.clone() {
                            let mut conn_handler = conn_handler_wrapper_b.lock().await;
                            conn_handler.handle_remote_response_finished(ctx, &cached_buf).await;
                            cached_buf.clear();
                        }

                        cached_ctx = Some(new_ctx);
                    }

                    if let Some(new_ctx) = cached_ctx.clone() {
                        if new_ctx.should_update_cache {
                            if let Some(sql) = new_ctx.sql.clone() {
                                if new_ctx.should_update_cache {
                                    cached_buf.extend_from_slice(&buf[..n]);
                                    // println!("cache response local");
                                }
                            }
                        }
                    }

                    let error_extra_msg = format!(
                        "client_writer write_all() fail.remote write to client.ctx:{:?},v:{:?}",
                        cached_ctx.clone(),
                        String::from_utf8_lossy(&buf[..n])
                    );
                    let mut client_writer = client_writer_lock.lock().await;
                    client_writer
                        .write_all(&buf[..n])
                        .await
                        .expect(&*error_extra_msg);
                }
                Err(e) => {
                    println!("remote_to_client error:{:#?}", e);
                    return Err(e);
                }
            }
        }
    };

    tokio::select! {
        result = client_to_remote => {
            if let Err(e) = result {
                eprintln!("Error transferring client to remote: {}", e);
            }
        }
        result = remote_to_client => {
            if let Err(e) = result {
                eprintln!("Error transferring remote to client: {}", e);
            }
        }
    }
}

// #[tokio::main]
// async fn main() -> async_io::Result<()> {
//     let listener = AsyncTcpListener::bind("0.0.0.0:10101").await?;
//
//     println!("Listening on {}", listener.local_addr()?);
//     let redis_client = redis::Client::open("redis://127.0.0.1").unwrap();
//
//     loop {
//         let redis_client = redis_client.clone();
//         let (client_stream, client_addr) = listener.accept().await?;
//
//         println!("Accepted connection from {}", client_addr);
//         tokio::spawn(async move {
//             // handle_client(client_stream, "172.22.240.1:3306".parse().unwrap(), conn).await;
//             let mut redis_conn = redis_client.clone().get_async_connection().await.unwrap();
//             let conn_handler = VirtDBConnectionHandler::new(redis_conn);
//             handle_client(
//                 client_stream,
//                 "127.0.0.1:3306".parse().unwrap(),
//                 conn_handler,
//             )
//                 .await;
//         });
//     }
// }


pub struct VirtDBConnectionHandler {
    pub redis_conn: Connection,
    dialect: MySqlDialect,
    pub server_config: VirtDBConfig,
    pub exec_log_channel_sender: Sender<ExecLog>,
    pub cache_load_task_channel_sender: Sender<CacheTaskInfo>,
}

impl VirtDBConnectionHandler {
    pub fn new(redis_conn: Connection,
               server_config: VirtDBConfig,
               exec_log_channel_sender: Sender<ExecLog>,
               cache_load_task_channel_sender: Sender<CacheTaskInfo>, ) -> VirtDBConnectionHandler {
        VirtDBConnectionHandler {
            redis_conn,
            dialect: MySqlDialect {},
            server_config,
            exec_log_channel_sender,
            cache_load_task_channel_sender,
        }
    }

    pub async fn handle_request(&mut self, mut ctx: &mut ProxyContext, packet_type: PacketType) -> Action {
        // println!("packet_type:{:?},sql:{:?}", packet_type.clone(), sql.clone());
        let mut redis_duration = 0;
        ctx.fn_start_time = Local::now();

        if let None = ctx.sql {
            return Action::FORWARD;
        }
        let sql = ctx.sql.clone().unwrap();
        let result_action = match packet_type {
            PacketType::ComQuery => {
                if !sql.clone().to_uppercase().starts_with("SELECT") {
                    return Action::FORWARD;
                }
                // println!("[match]sql:{:?}", sql);
                let tmp_sql = sql.clone();
                if tmp_sql.to_uppercase().contains("\0\0\0\u{3}") {
                    // println!("sqls:{:?}", tmp_sql);
                    return Action::FORWARD;
                }
                // println!("[ComQuery]sql:{:?}", sql);
                if !sql.to_uppercase().starts_with("SELECT") {
                    // println!("continue1");
                    return Action::FORWARD;
                }

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
                    return Action::FORWARD;
                }

                let redis_get_start_time = Local::now();
                let cache_key = format!("cache:\"{}\"", sql);
                let cache_exists_check_result: RedisResult<bool> = self.redis_conn.exists(cache_key.clone()).await;
                if let Err(err) = cache_exists_check_result {
                    // println!("continue2");
                    return Action::FORWARD;
                }
                let is_exists = cache_exists_check_result.unwrap();
                if !is_exists {
                    ctx.should_update_cache = true;
                    ctx.cache_duration = cache_config_entity_option.unwrap().duration;
                    // println!("continue3");
                    return Action::FORWARD;
                }

                let cache_v_result: RedisResult<Vec<u8>> =
                    self.redis_conn.get(cache_key).await;
                if let Err(err) = cache_v_result {
                    // println!("continue4");
                    return Action::FORWARD;
                }

                let mut cache_v = cache_v_result.unwrap();

                if cache_v.len() < 1 {
                    // println!("continue5");
                    return Action::FORWARD;
                }

                redis_duration = (Local::now() - redis_get_start_time).num_milliseconds();
                trace!("[handle_request]redis_v:{:?}", cache_v);
                ctx.redis_duration = redis_duration;
                ctx.from_cache = true;

                Action::RESPONSED(cache_v)
            }
            PacketType::ComStmtExecute => {
                println!("stmt,sql:{:?}", sql);
                return Action::FORWARD;
            }
            PacketType::ComQuit => {
                // return Ok(());
                return Action::FORWARD;
            }
            _ => {
                return Action::FORWARD;
            }
        };
        let mysql_exec_start_time = Local::now();
        ctx.mysql_exec_start_time = Some(mysql_exec_start_time);
        result_action
    }

    pub fn handle_response(&mut self) {}

    pub async fn handle_remote_response_finished(&mut self, ctx: ProxyContext, full_response: &Vec<u8>) {
        if let None = ctx.sql {
            return;
        }
        let mysql_duration = (Local::now() - ctx.mysql_exec_start_time.unwrap_or_default()).num_milliseconds();

        let sql = ctx.sql.unwrap();
        if ctx.should_update_cache {
            // let cache_key = format!("cache:\"{}\"", sql.clone());
            // let cache_v = full_response.as_slice();
            // println!("save remote response.sql:{:?},v:{:?}", sql.clone(),String::from_utf8_lossy(cache_v));
            //
            // let _: RedisResult<Vec<u8>> = self.redis_conn
            //     .set(cache_key.clone(), cache_v.clone())
            //     .await;
            //or
            let cache_v = full_response.as_slice().to_vec();
            let send_result = self.cache_load_task_channel_sender.send(CacheTaskInfo::new(sql.clone(), cache_v, ctx.cache_duration)).await;
            match send_result {
                Ok(_) => {}
                Err(err) => {
                    warn!("Send ExecLog fail.err:{:?}",err);
                }
            }
        }

        let total_duration = (Local::now() - ctx.fn_start_time).num_milliseconds();
        //只记录select
        if let Some(sql_pattern) = sql_to_pattern(sql.clone().as_str()) {
            let exec_log = ExecLog {
                sql_str: sql_pattern,
                total_duration,
                mysql_duration,
                redis_duration: ctx.redis_duration,
                from_cache: ctx.from_cache,
            };
            let send_result = self.exec_log_channel_sender.send(exec_log).await;
            match send_result {
                Ok(_) => {}
                Err(err) => {
                    warn!("Send ExecLog fail.err:{:?}",err);
                }
            }
        }
    }
}