use std::collections::HashMap;
use std::env::Args;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::ops::Deref;
use std::sync::{Arc, LockResult, Mutex, MutexGuard, TryLockResult};
use std::time::Duration;

use chrono::Local;
use itertools::Itertools;
use log::{debug, info};
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_util::MetricKindMask;
use once_cell::sync::Lazy;
use reqwest::{Body, Client, ClientBuilder, Error, Response};
use serde::{Deserialize, Serialize};
use tokio::time;
use tokio::time::{Instant, Interval};

use crate::math::avg::AveragedCollection;
use crate::sys_config;
use crate::sys_config::VirtDBConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataWrapper<V> {
    pub code: i32,
    pub message: String,
    pub success: bool,
    pub data: Option<V>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq,PartialEq)]
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

static EXEC_LOG_LIST_MUTEX: Lazy<Mutex<Vec<ExecLog>>> = Lazy::new(|| {
    Mutex::new(Vec::new())
});

pub fn get_exec_log_list_copy() -> Option<Vec<ExecLog>> {
    let locked_result = &EXEC_LOG_LIST_MUTEX.lock();
    return match locked_result {
        Ok(exec_log_list) => {
            let cloned_list = exec_log_list.iter()
                .map(|ele| {
                    ele.clone()
                })
                .collect();
            Some(cloned_list)
        }
        Err(err) => {
            warn!("get lock fail.{:?}",err);
            None
        }
    };
}

pub async fn record_exec_log(exec_log: ExecLog) {
    let locked_result = EXEC_LOG_LIST_MUTEX.lock();
    match locked_result {
        Ok(mut exec_log_list) => {
            info!("add new exec_log:{:?}",exec_log);
            exec_log_list.push(exec_log);
        }
        Err(err) => {
            warn!("Get EXEC_LOG_LIST fail:{:?}",err);
        }
    };
}

pub async fn remove_exec_logs(expired_exec_log_list: &Vec<ExecLog>) {
    let locked_result = EXEC_LOG_LIST_MUTEX.lock();
    match locked_result {
        Ok(mut exec_log_list) => {
            exec_log_list.drain_filter(|ele| {
                for expired_exec_log in expired_exec_log_list {
                    if ele == expired_exec_log {
                        return true;
                    }
                }
                return false;
            });
        }
        Err(err) => {
            warn!("Get EXEC_LOG_LIST fail:{:?}",err);
        }
    };
}


pub async fn enable_node_live_refresh_job(sys_config: VirtDBConfig) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(1));
        interval.tick().await;
        info!("node_live_refresh_job started.");
        loop {
            interval.tick().await;
            //样板结束

            let exec_log_list_result = get_exec_log_list_copy();
            if let None = exec_log_list_result {
                return;
            }
            let exec_log_list = exec_log_list_result.unwrap();

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
                        avg_calculator.add(exec_log.total_duration as i32);
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
                        return;
                    }
                    let data_wrapper_result = response.json::<DataWrapper<String>>()
                        .await;
                    match data_wrapper_result {
                        Ok(data_wrapper) => {
                            if !data_wrapper.success {
                                warn!("register vt_node fail. response:{:?}",data_wrapper);
                            }
                            //success
                            remove_exec_logs(&exec_log_list).await;
                        }
                        Err(e) => {
                            warn!("register vt_node fail:{:?}",e);
                        }
                    };
                }
                Err(e) => {
                    warn!("register vt_node fail:{:?}",e);
                }
            };
            //样板开始
            time::sleep(Duration::from_secs(10)).await;
        }
    });
}

pub fn enable_metrics(sys_config: VirtDBConfig) {
    // tracing_subscriber::fmt::init();

    let builder = PrometheusBuilder::new();
    let addr = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), sys_config.metric.expose_port);
    builder
        .with_http_listener(addr)
        .idle_timeout(
            MetricKindMask::COUNTER | MetricKindMask::HISTOGRAM,
            Some(Duration::from_secs(10)),
        )
        .install()
        .expect("failed to install Prometheus recorder");
    info!("prometheus exposed at 0.0.0.0:{}",sys_config.metric.expose_port);
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

    let request_body = serde_json::to_string_pretty(&params).unwrap();
    debug!("[job]request body:{}",request_body);
    let response = Client::builder()
        .no_proxy()
        .build()?
        .post(register_api_url)
        .header("Content-Type", "application/json")
        .body(request_body)
        .send()
        .await?;
    return Ok(response);
}