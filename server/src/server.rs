
use crate::mysql_protocol::{MySQL};
use log::{error};
use opensrv_mysql::*;
use tokio::net::TcpListener;

pub async fn start() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("0.0.0.0:3307").await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let (r, w) = stream.into_split();
        tokio::spawn(async move {
            //TODO pool
            let mysql_url = "mysql://root:root@127.0.0.1:3306";
            let redis_url = "redis://127.0.0.1/";

            let r = AsyncMysqlIntermediary::run_on(MySQL::new(mysql_url, redis_url), r, w).await;
            trace!("mysql end result:{:?}", r);
            return r;
        });
    }
}
