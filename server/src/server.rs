#![allow(unused_imports)]

use std::sync::{Arc, Mutex};

use log::kv::ToValue;
use mysql_async::Conn;
use mysql_common::bigdecimal03::ToPrimitive;
use opensrv_mysql::*;
use tokio::net::TcpListener;

use crate::mysql_protocol::MySQL;
use crate::sys_config::VirtDBConfig;

pub async fn start(sys_config: VirtDBConfig) -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = format!("0.0.0.0:{:?}", sys_config.server.port.clone());
    let listener = TcpListener::bind(server_addr).await?;
    let mutex = Arc::new(Mutex::new(0));

    loop {
        let (stream, _) = listener.accept().await?;
        let (r, w) = stream.into_split();

        let server_config = sys_config.server.clone();
        let mysql_config = sys_config.mysql.clone();
        let redis_config = sys_config.redis.clone();
        let mut locked_value = mutex.lock().unwrap();
        *locked_value += 1;
        let conn_id = locked_value.to_u32().unwrap();

        tokio::spawn(async move {
            //TODO pool mysql+redis ?
            let mysql_username = mysql_config.username;
            let mysql_password = mysql_config.password;
            let mysql_ip = mysql_config.ip;
            let mysql_port = mysql_config.port;

            let redis_ip = redis_config.ip;
            let redis_port = redis_config.port;
            let redis_requirepass = redis_config.requirepass;

            let mysql_url = format!(
                "mysql://{}:{}@{}:{}",
                mysql_username, mysql_password, mysql_ip, mysql_port
            );
            let redis_url = format!("redis://{}@{}:{}", redis_requirepass, redis_ip, redis_port);
            debug!("mysql_url:{:?},redis_url:{:?}", mysql_url, redis_url);

            let mysql_conn = Conn::from_url(mysql_url.to_string()).await?;
            let client = redis::Client::open(redis_url.to_string())?;
            let redis_conn = client.get_async_connection().await?;

            let r = AsyncMysqlIntermediary::run_on(
                MySQL::new(mysql_conn, redis_conn, conn_id, server_config),
                r,
                w,
            )
            .await;
            if r.is_err() {
                warn!("mysql connection finished with error:{:?}", r);
            }
            return r;
        });
    }
}
