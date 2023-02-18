use std::{io, thread, time};
use std::collections::HashMap;
use std::env::Args;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::ops::Deref;
use std::sync::{Arc, LockResult, Mutex, MutexGuard, TryLockResult};
use std::thread::sleep;
use std::time::Duration;

use chrono::{DateTime, Local};
use crossbeam::channel::Receiver;
use itertools::Itertools;
use log::{debug, info};
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_util::MetricKindMask;
use once_cell::sync::Lazy;
use redis::{Commands, RedisResult};
use reqwest::blocking::Response;
use reqwest::Error;
use serde::{Deserialize, Serialize};
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::Parser;

use crate::{math, sys_config, utils};
use crate::math::avg::AveragedCollection;
use crate::sys_config::VirtDBConfig;
use crate::utils::sys_path;

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
    body: String,
    duration: i32,
}

impl CacheTaskInfo {
    pub fn new(sql: String, body: String, duration: i32) -> CacheTaskInfo {
        CacheTaskInfo {
            sql,
            body,
            duration,
        }
    }
}

static CACHE_TASK_INFO_LIST_MUTEX: Lazy<Mutex<Vec<CacheTaskInfo>>> = Lazy::new(|| {
    Mutex::new(Vec::new())
});

pub fn add_cache_task(cache_task_info: CacheTaskInfo) {
    match CACHE_TASK_INFO_LIST_MUTEX.lock() {
        Ok(mut list) => {
            list.push(cache_task_info);
        }
        Err(err) => {
            warn!("get lock fail.{:?}",err);
        }
    };
}

pub fn get_cache_task_info_list_copy() -> Option<Vec<CacheTaskInfo>> {
    let locked_result = &CACHE_TASK_INFO_LIST_MUTEX.lock();
    return match locked_result {
        Ok(list) => {
            let cloned_list = list.iter()
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

pub fn enable_metric_writing_job(sys_config: VirtDBConfig, channel_receiver: Receiver<ExecLog>) {
    thread::spawn(move || {
        info!("metric data writing task started.");
        loop {
            sleep(Duration::from_secs(5));
            let exec_log_list:Vec<ExecLog> = channel_receiver.try_iter()
                .collect();

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

            let result = register(&sys_config, metric_history_list);
            let _ = match result {
                Ok(response) => {
                    if response.status().as_u16() != 200 {
                        warn!("register vt_node fail. response:{:?}",response);
                    }else{
                        let data_wrapper_result = response.json::<DataWrapper<String>>();
                        match data_wrapper_result {
                            Ok(data_wrapper) => {
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
        };
    });
}

pub fn enable_cache_task_handle_job(sys_config: VirtDBConfig) {
    let redis_config = sys_config.clone().redis;
    let redis_ip = redis_config.ip;
    let redis_port = redis_config.port;
    let redis_requirepass = redis_config.requirepass;

    let redis_url = format!("redis://{}@{}:{}", redis_requirepass, redis_ip, redis_port);
    let redis_client = redis::Client::open(redis_url.to_string()).unwrap();
    let mut redis_conn = redis_client.get_connection().unwrap();
    thread::spawn(move || {
        info!("cache handle task started.");
        loop {
            let task_info_list = get_cache_task_info_list_copy();
            if task_info_list.is_none() {
                continue;
            }
            let task_info_list = task_info_list.unwrap();
            for cache_task_info in task_info_list.into_iter() {
                let sql = cache_task_info.sql;
                let redis_key = format!("cache:{:?}", sql);
                let redis_v = cache_task_info.body;
                let cache_duration = cache_task_info.duration;

                let rv: RedisResult<Vec<u8>> = redis_conn
                    .set_ex(
                        redis_key.clone(),
                        redis_v,
                        cache_duration as usize,
                    );
                if let Err(err) = rv {
                    match err.code() {
                        None => {}
                        Some(err_code) => {
                            warn!("redis set cmd fail. for sql:{:?},err:{:?}",sql,err_code)
                        }
                    }
                }
            }
        };
    });
}


static HTTP_CLIENT: Lazy<reqwest::blocking::Client> = Lazy::new(|| {
    reqwest::blocking::Client::new()
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

fn register(sys_config: &VirtDBConfig, metric_history_list: Vec<MetricHistory>) -> Result<Response, Error> {
    let address = sys_config.admin.address.clone();
    let register_api_url = format!("{}/vt_node/register", address);
    let params = VtNodeRegisterParam {
        port: sys_config.server.port.to_string(),
        metric_history_list,
    };

    let request_body = serde_json::to_string(&params).unwrap();
    debug!("[job]request body:{}",request_body);
    let response = HTTP_CLIENT
        .post(register_api_url)
        .header("Content-Type", "application/json")
        .body(request_body)
        .send()?;
    return Ok(response);
}