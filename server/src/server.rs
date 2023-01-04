
use crate::mysql_protocol::{MySQL};

use opensrv_mysql::*;
use tokio::net::TcpListener;
use crate::sys_config::VirtDBConfig;

pub async fn start(sys_config: VirtDBConfig) -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = format!("0.0.0.0:{:?}", sys_config.server.port.clone());
    let listener = TcpListener::bind(server_addr).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let (r, w) = stream.into_split();

        let mysql_config = sys_config.mysql.clone();
        let redis_config = sys_config.redis.clone();
        tokio::spawn(async move {
            //TODO pool mysql+redis ?
            let mysql_username = mysql_config.username;
            let mysql_password = mysql_config.password;
            let mysql_ip = mysql_config.ip;
            let mysql_port = mysql_config.port;

            let redis_ip = redis_config.ip;
            let redis_port = redis_config.port;
            let redis_requirepass = redis_config.requirepass;

            let mysql_url = format!("mysql://{}:{}@{}:{}",mysql_username,mysql_password,mysql_ip,mysql_port);
            let redis_url = format!("redis://{}@{}:{}",redis_requirepass,redis_ip,redis_port);
            info!("mysql_url:{:?},redis_url:{:?}",mysql_url,redis_url);
            let r = AsyncMysqlIntermediary::run_on(MySQL::new(&*mysql_url, redis_url), r, w).await;
            trace!("mysql end result:{:?}", r);
            return r;
        });
    }
}
