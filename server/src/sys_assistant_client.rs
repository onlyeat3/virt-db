use std::cmp::max;
use std::future::Future;
use std::sync::{Mutex};
use std::time::Duration;

use chrono::{Local};
use itertools::Itertools;
use log::{debug, info};
use once_cell::sync::Lazy;
use redis::{AsyncCommands, Client, RedisResult};
use reqwest::{Error, Response};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;

use crate::math::avg::AveragedCollection;
use crate::sys_config::VirtDBConfig;
use crate::sys_redis::SysRedisClient;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ExecLog {
    pub sql_str: String,
    pub total_duration: i64,
    pub mysql_duration: i64,
    pub redis_duration: i64,
    pub from_cache: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MetricHistory {
    pub sql_str: String,
    pub db_server_port: String,
    pub database_name: String,
    pub avg_duration: i32,
    pub max_duration: i64,
    pub min_duration: i64,
    pub exec_count: usize,
    pub cache_hit_count: i32,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct CacheTaskInfo {
    sql: String,
    body: Vec<u8>,
    duration: i32,
}

impl CacheTaskInfo {
    pub fn new(sql: String, body: Vec<u8>, duration: i32) -> CacheTaskInfo {
        CacheTaskInfo {
            sql,
            body,
            duration,
        }
    }
}

pub async fn handle_messages<T, F>(mut rx: mpsc::Receiver<T>, sys_config: VirtDBConfig, handler: impl Fn(Vec<T>, VirtDBConfig) -> F)
    where
        F: Future<Output=()>,
{
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        let mut messages = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }

        if !messages.is_empty() {
            handler(messages, sys_config.clone()).await;
        }
    }
}

pub async fn handle_metrics(exec_log_list: Vec<ExecLog>, sys_config: VirtDBConfig) {
    let metric_history_list = exec_log_list.iter()
        .sorted_by_key(|v| v.sql_str.clone())
        .group_by(|v| v.sql_str.to_string())
        .into_iter()
        .map(|(sql_str, group)| {
            let mut avg_calculator = AveragedCollection::new();
            let mut max_duration = -1;
            let mut min_duration = i64::MAX;
            let mut cache_hit_count = 0;
            let mut total_count = 0;
            let list = group.into_iter()
                .map(|v| v.to_owned())
                .collect::<Vec<ExecLog>>();

            for exec_log in list {
                avg_calculator.add(exec_log.total_duration);
                if max_duration < exec_log.total_duration {
                    max_duration = exec_log.total_duration;
                }
                if min_duration > exec_log.total_duration {
                    min_duration = exec_log.total_duration;
                }

                if exec_log.from_cache {
                    cache_hit_count += 1;
                }
                total_count += 1;
            }

            let metric_history = MetricHistory {
                sql_str,
                db_server_port: sys_config.server.port.to_string(),
                database_name: "".to_string(),//TODO
                avg_duration: avg_calculator.average() as i32,
                max_duration,
                min_duration,
                exec_count: total_count,
                cache_hit_count: cache_hit_count as i32,
                created_at: Local::now().timestamp(),
            };
            metric_history
        })
        .collect::<Vec<MetricHistory>>();

    let result = register(&sys_config, metric_history_list).await;
    let _ = match result {
        Ok(response) => {
            if response.status().as_u16() != 200 {
                warn!("register vt_node fail. response:{:?}",response);
            } else {
                let data_wrapper_result = response.json::<DataWrapper<String>>().await;
                match data_wrapper_result {
                    Ok(data_wrapper) => {
                        debug!("handle_metrics. response:{:?}",data_wrapper);
                        if !data_wrapper.success {
                            warn!("register vt_node fail. response:{:?}",data_wrapper);
                        }
                    }
                    Err(e) => {
                        warn!("register vt_node fail:{:?}",e);
                    }
                };
            }
        }
        Err(e) => {
            warn!("register vt_node fail:{:?}",e);
        }
    };
}

pub fn enable_metric_writing_job(sys_config: VirtDBConfig, channel_receiver: Receiver<ExecLog>) {
    info!("metric data writing task started.");
    tokio::spawn(async move {
        handle_messages(channel_receiver, sys_config, handle_metrics).await;
    });
}

pub fn enable_cache_task_handle_job(sys_config: VirtDBConfig, cache_load_task_channel_receiver: Receiver<CacheTaskInfo>) {
    let nodes = sys_config.redis.nodes;
    info!("cache handle task started.");
    tokio::spawn(async move {
        let nodes = nodes.clone();

        let mut cache_load_task_channel_receiver = cache_load_task_channel_receiver;
        // let mut sys_redis_client = SysRedisClient::new(nodes.as_str()).unwrap();
        let client = Client::open(nodes.as_str()).unwrap();
        let mut redis_conn = client.clone().get_async_connection().await.unwrap();

        loop {
            if let Some(cache_task_info) = cache_load_task_channel_receiver.recv().await {
                let sql = cache_task_info.sql;
                let redis_key = format!("cache:{:?}", sql);
                let redis_v = cache_task_info.body;
                let cache_duration = cache_task_info.duration;
                let cache_duration = max(60,cache_duration);
                debug!("[cache_task_handle_job]sql:{:?},redis_key:{:?},cache_duration:{:?}",sql.clone(),redis_key,cache_duration);

                let rv: RedisResult<()> = redis_conn
                    .set_ex(
                        &*redis_key.clone(),
                        redis_v,
                        cache_duration as usize,
                    ).await;
                if let Err(err) = rv {
                    warn!("redis set cmd fail. for sql:{:?},err:{:?}",sql,err);
                    // match err.code() {
                    //     None => {}
                    //     Some(err_code) => {
                    //         warn!("redis set cmd fail. for sql:{:?},err:{:?}",sql,err_code)
                    //     }
                    // }
                }
            }
        };
    });
}


static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::new()
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataWrapper<V> {
    pub code: i32,
    pub message: String,
    pub success: bool,
    pub data: Option<V>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VtNodeRegisterParam {
    pub port: String,
    pub metric_history_list: Vec<MetricHistory>,
}

async fn register(sys_config: &VirtDBConfig, metric_history_list: Vec<MetricHistory>) -> Result<Response, Error> {
    let address = sys_config.admin.address.clone();
    let register_api_url = format!("{}/vt_node/register", address);
    let params = VtNodeRegisterParam {
        port: sys_config.server.port.to_string(),
        metric_history_list,
    };

    let request_body = serde_json::to_string(&params).unwrap();
    debug!("[job]request body:{}",request_body);
    HTTP_CLIENT
        .post(register_api_url)
        .header("Content-Type", "application/json")
        .body(request_body)
        .send().await
}