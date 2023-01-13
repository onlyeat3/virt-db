use std::thread;
use std::time::Duration;
use chrono::Local;

use log::{debug, info};
use tokio::{runtime, time};
use tokio::time::Instant;

use crate::AppState;

//启用vt-server存活状态检查任务
pub async fn enable_vt_node_alive_check(app_state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(1));
        interval.tick().await;
        let start = Instant::now();
        info!("vt_node_alive_check started.");
        loop {
            interval.tick().await;
            let mut vt_nodes = app_state.vt_nodes_lock.lock().await;
            vt_nodes.retain(|key,expire_at|{
                &Local::now() < expire_at
            });
            for entry in vt_nodes.iter() {
                debug!("current vt_node:{:?}",entry)
            }
            time::sleep(Duration::from_secs(1)).await;
            debug!("check validity .time:{:?}", start.elapsed());
        }
    });
}