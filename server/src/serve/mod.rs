// use byteorder::{LittleEndian, ReadBytesExt as BorderReadBytesExt};
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use chrono::{DateTime, Local};
// use mysql_common::proto::codec::CompDecoder::Packet;

use redis::aio::{Connection};
use redis::{AsyncCommands, RedisResult};
use sqlparser::dialect::MySqlDialect;
use tokio::io as async_io;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener as AsyncTcpListener, TcpStream as AsyncTcpStream};
use tokio::sync::{mpsc, Mutex, MutexGuard};
use tokio::sync::mpsc::Sender;
use crate::meta::CacheConfigEntity;
// use crate::protocol::{Packet, PacketType};
use crate::sys_assistant_client::{CacheTaskInfo, ExecLog};
use crate::sys_config::VirtDBConfig;
use crate::{meta, utils};
use crate::protocol::{Packet, PacketType};
use crate::utils::sys_sql::sql_to_pattern;

const BUFFER_SIZE: usize = 8 * 1024;

pub enum Action {
    FORWARD,
    DROP,
    RESPONSED(Vec<u8>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProxyContext {
    pub sql: Option<String>,
    pub should_update_cache: bool,
    pub fn_start_time: Instant,
    pub mysql_exec_start_time: Option<Instant>,
    pub redis_duration: i64,
    pub from_cache: bool,
    pub cache_duration: i32,
    pub total_duration: i64,
    pub mysql_duration: i64,
    pub skip: bool,//不做任何处理,纯代理
}

pub async fn handle_client(
    mut client_stream: AsyncTcpStream,
    remote_addr: SocketAddr,
    conn_handler: VirtDBConnectionHandler,
) {
    let mut remote_stream = match AsyncTcpStream::connect(remote_addr).await {
        Ok(stream) => stream,
        Err(e) => {
            info!("Failed to connect to {}: {}", remote_addr, e);
            return;
        }
    };

    let (mut client_reader, client_writer) = client_stream.split();
    let (mut remote_reader, mut remote_writer) = remote_stream.split();

    let (ctx_sender, mut ctx_receiver) = mpsc::channel(1);

    // let client_writer_semaphore = Semaphore::new(1);
    let client_writer_lock = Arc::new(Mutex::new(client_writer));
    let client_writer_lock_a = client_writer_lock.clone();

    let conn_handler = Arc::new(Mutex::new(conn_handler));
    let conn_handler_wrapper_a = conn_handler.clone();
    let conn_handler_wrapper_b = conn_handler.clone();

    let client_to_remote = async move {
        let mut buf = [0; BUFFER_SIZE];
        let mut r_buf = ReadBuf::new(&mut buf);
        let conn_handler_wrapper_a = conn_handler_wrapper_a;
        loop {
            match client_reader.read_buf(&mut r_buf).await {
                Ok(n) => {
                    if n == 0 {
                        return Ok(());
                    }
                    let mut ctx = ProxyContext {
                        sql: None,
                        should_update_cache: false,
                        fn_start_time: Instant::now(),
                        mysql_exec_start_time: None,
                        redis_duration: 0,
                        from_cache: false,
                        cache_duration: 0,
                        total_duration: 0,
                        mysql_duration: 0,
                        skip: false,
                    };
                    let data = r_buf.filled();

                    info!("data:{:?}",String::from_utf8_lossy(data));
                    let packet = Packet::new(data.to_vec());
                    if let Err(_) = packet.packet_type() {
                        remote_writer.write_all(data).await?;
                        continue;
                    }
                    let packet_type = packet.packet_type().unwrap();
                    let bytes = &data[5..n];
                    let sql_result = String::from_utf8(bytes.to_vec());
                    if let Ok(sql) = sql_result {
                        ctx.sql = Some(sql.clone());
                        // info!("current sql:{:?}",sql);
                    }
                    let mut conn_handler = conn_handler_wrapper_a.lock().await;
                    let action = conn_handler.handle_request(&mut ctx, packet_type).await;

                    let skip = match action {
                        Action::FORWARD => {
                            ctx.mysql_exec_start_time = Some(Instant::now());
                            // info!("before send. current sql:{:?}",ctx.sql.clone());
                            ctx_sender.send(ctx).await.expect("send ctx fail");
                            false
                        }
                        Action::DROP => true,
                        Action::RESPONSED(mut bytes) => {
                            let mut client_writer = client_writer_lock_a.lock().await;
                            // println!("sql:{:?},value from cache:true", sql.clone());
                            // println!("sql:{:?},cache_v:{:X?}", sql.clone(), String::from_utf8_lossy(&*cache_v.clone()));
                            let r = client_writer.write_all(bytes.as_mut_slice()).await;
                            if let Err(err) = r {
                                info!("write to client fail.err:{:?}", err);
                            }
                            true
                        }
                    };

                    if skip {
                        r_buf.clear();
                        continue;
                    }

                    // println!("Received from client: {:?},type:{:#?}", String::from_utf8_lossy(&*buf[..n].to_vec()), &buf[0]);
                    remote_writer.write_all(data).await?;
                    r_buf.clear();
                }
                Err(e) => {
                    info!("client_to_remote:{:#?}", e);
                    return Err(e);
                }
            }
        }
    };

    let client_writer_lock_b = client_writer_lock.clone();
    let remote_to_client = async move {
        let mut buf = [0; BUFFER_SIZE];
        let mut r_buf = ReadBuf::new(&mut buf);
        let mut cached_buf = vec![];
        let mut cached_ctx: Option<Arc<Mutex<ProxyContext>>> = None;
        let client_writer_lock = client_writer_lock_b;
        let conn_handler_wrapper_b = conn_handler_wrapper_b;
        loop {
            match remote_reader.read_buf(&mut r_buf).await {
                Ok(n) => {
                    if n == 0 {
                        return Ok(());
                    }
                    let data = r_buf.filled();

                    // info!("Received from remote: {:X?}", String::from_utf8_lossy(data).to_string());

                    if let Ok(current_ctx) = ctx_receiver.try_recv() {
                        //清理上次保存的记录
                        // info!("recv new ctx:{:?},mysql_start_time:{:?},old exists:{:?}", current_ctx.sql,current_ctx.mysql_exec_start_time, cached_ctx.is_some());
                        if let Some(old_ctx) = cached_ctx.clone() {
                            let mut conn_handler = conn_handler_wrapper_b.lock().await;
                            conn_handler.handle_remote_response_finished(old_ctx, &cached_buf).await;
                            cached_buf.clear();
                        }
                        // info!("new ctx:{:?}", current_ctx);
                        cached_ctx = Some(Arc::new(Mutex::new(current_ctx)));
                    }

                    cached_buf.extend_from_slice(data);

                    if let Some(mut old_ctx) = cached_ctx.clone() {
                        let old_ctx = old_ctx.lock().await;
                        // if old_ctx.should_update_cache {
                        //     if let Some(_) = old_ctx.sql.clone() {
                        //         if old_ctx.should_update_cache {
                        //             cached_buf.extend_from_slice(data);
                        //             // println!("cache response local");
                        //         }
                        //     }
                        // }
                        //handle partial response
                        let mut conn_handler = conn_handler_wrapper_b.lock().await;
                        conn_handler.handle_response(old_ctx);
                    }


                    let error_extra_msg = format!(
                        "client_writer write_all() fail.remote write to client.ctx:{:?},v:{:?}",
                        cached_ctx.clone(),
                        String::from_utf8_lossy(data)
                    );
                    let mut client_writer = client_writer_lock.lock().await;
                    client_writer
                        .write_all(data)
                        .await
                        .expect(&*error_extra_msg);

                    r_buf.clear();
                }
                Err(e) => {
                    info!("remote_to_client error:{:#?}", e);
                    return Err(e);
                }
            }
        }
    };

    tokio::select! {
        result = client_to_remote => {
            if let Err(e) = result {
                error!("Error transferring client to remote: {}", e);
            }
        }
        result = remote_to_client => {
            if let Err(e) = result {
                error!("Error transferring remote to client: {}", e);
            }
        }
    }
}

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
        ctx.fn_start_time = Instant::now();

        if let None = ctx.sql {
            return Action::FORWARD;
        }
        let origin_sql = ctx.sql.clone().unwrap();
        let sql = utils::sys_sql::remove_comments(origin_sql.clone());
        info!("origin_sql:{:?},sql:{:?}",origin_sql, sql.clone());
        let result_action = match packet_type {
            PacketType::ComQuery => {
                if !sql.clone().to_uppercase().starts_with("SELECT") {
                    return Action::FORWARD;
                }
                let tmp_sql = sql.clone();
                if tmp_sql.to_uppercase().contains("\0\0\0\u{3}") {
                    trace!("sqls:{:?}", tmp_sql);
                    ctx.skip = true;
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

                let redis_get_start_time = Instant::now();
                let cache_key = format!("cache:\"{}\"", sql);
                let cache_exists_check_result: RedisResult<bool> = self.redis_conn.exists(cache_key.clone()).await;
                if let Err(_) = cache_exists_check_result {
                    // println!("continue2");
                    ctx.cache_duration = cache_config_entity_option.unwrap().duration;
                    ctx.redis_duration = (Instant::now() - redis_get_start_time).as_millis() as i64;
                    return Action::FORWARD;
                }
                let is_exists = cache_exists_check_result.unwrap();
                if !is_exists {
                    ctx.should_update_cache = true;
                    ctx.cache_duration = cache_config_entity_option.unwrap().duration;
                    ctx.redis_duration = (Instant::now() - redis_get_start_time).as_millis() as i64;
                    // println!("continue3");
                    return Action::FORWARD;
                }

                let cache_v_result: RedisResult<Vec<u8>> =
                    self.redis_conn.get(cache_key).await;
                if let Err(_) = cache_v_result {
                    // println!("continue4");
                    ctx.cache_duration = cache_config_entity_option.unwrap().duration;
                    ctx.redis_duration = (Instant::now() - redis_get_start_time).as_millis() as i64;
                    return Action::FORWARD;
                }

                let cache_v = cache_v_result.unwrap();

                if cache_v.len() < 1 {
                    // println!("continue5");
                    ctx.cache_duration = cache_config_entity_option.unwrap().duration;
                    ctx.redis_duration = (Instant::now() - redis_get_start_time).as_millis() as i64;
                    return Action::FORWARD;
                }

                trace!("[handle_request]redis_v:{:?}", cache_v);
                ctx.redis_duration = (Instant::now() - redis_get_start_time).as_millis() as i64;
                ctx.from_cache = true;

                Action::RESPONSED(cache_v)
            }
            PacketType::ComStmtExecute => {
                info!("stmt,sql:{:?}", sql);
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
        result_action
    }

    //处理大数据包拆分的单个数据包
    pub fn handle_response(&mut self, mut ctx: MutexGuard<ProxyContext>) {
        // info!("mysql_exec_start_time:{:?}",ctx.mysql_exec_start_time);
        ctx.total_duration = (Instant::now() - ctx.fn_start_time).as_millis() as i64;

        // info!("fn_start_time:{:?},total_duration:{:?}",ctx.fn_start_time,ctx.total_duration);
        if let Some(mysql_exec_start_time) = ctx.mysql_exec_start_time {
            let mysql_duration = (Instant::now() - mysql_exec_start_time).as_millis() as i64;
            ctx.mysql_duration = mysql_duration;
        }
    }

    pub async fn handle_remote_response_finished(&mut self, ctx: Arc<Mutex<ProxyContext>>, full_response: &Vec<u8>) {
        let ctx = ctx.lock().await;
        // info!("handle_remote_response_finished,sql:{:?},total_duration:{:?},mysql_duration:{:?},redis_duration:{:?},start_time:{:?}",ctx.sql,ctx.total_duration,ctx.mysql_duration,ctx.redis_duration,ctx.mysql_exec_start_time);
        if let None = ctx.sql {
            return;
        }
        if let None = ctx.mysql_exec_start_time {
            return;
        }
        let mysql_duration = ctx.mysql_duration;
        let total_duration = ctx.total_duration;

        // info!("sql:{:?},mysql_duration:{:?},redis_duration:{:?},mysql_exec_start_time:{:?},total_duration:{:?}",ctx.sql.clone(),mysql_duration,ctx.redis_duration,ctx.mysql_exec_start_time,total_duration);

        let sql = ctx.sql.clone().unwrap();
        if ctx.should_update_cache && !ctx.skip {
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