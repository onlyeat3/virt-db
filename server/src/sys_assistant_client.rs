use std::collections::HashMap;
use std::env::Args;
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::ops::Deref;
use std::sync::{Arc, LockResult, Mutex, MutexGuard, TryLockResult};
use std::time::Duration;

use chrono::{DateTime, Local};
use itertools::Itertools;
use log::{debug, info};
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_util::MetricKindMask;
use once_cell::sync::Lazy;
use reqwest::{Body, Client, ClientBuilder, Error, Response};
use serde::{Deserialize, Serialize};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::time;
use tokio::time::{Instant, Interval};

use crate::math::avg::AveragedCollection;
use crate::{math, sys_config, utils};
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

pub fn record_exec_log(exec_log: ExecLog) {
    let locked_result = EXEC_LOG_LIST_MUTEX.lock();
    match locked_result {
        Ok(mut exec_log_list) => {
            trace!("add new exec_log:{:?}",exec_log);
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
            exec_log_list.retain(|ele| {
                for expired_exec_log in expired_exec_log_list {
                    if ele == expired_exec_log {
                        return false;
                    }
                }
                return true;
            });
        }
        Err(err) => {
            warn!("Get EXEC_LOG_LIST fail:{:?}",err);
        }
    };
}


pub async fn enable_metric_writing_job(sys_config: VirtDBConfig) {
    tokio::spawn(async move {
        info!("metric data writing task started.");

        let mut origin_now_str = utils::sys_datetime::now_str_data_file_name();
        let target_file = sys_path::resolve_as_current_path(format!("{}.json", origin_now_str)).unwrap();
        let target_file = OpenOptions::new()
            .write(true)
            .append(true)
            .create(true)
            .open(target_file)
            .await
            .unwrap();
        let mut writer = BufWriter::new(target_file);

        loop {
            time::sleep(Duration::from_secs(1)).await;

            let now_str = utils::sys_datetime::now_str_data_file_name();
            if now_str != origin_now_str {
                let target_file = sys_path::resolve_as_current_path(format!("{}.json", origin_now_str));
                if target_file.is_none() {
                    error!("Can not get current exe path");
                    continue;
                }
                let target_file = target_file.unwrap();
                let target_file = OpenOptions::new()
                    .write(true)
                    .append(true)
                    .create(true)
                    .open(target_file)
                    .await;
                if target_file.is_err() {
                    let err = target_file.err().unwrap();
                    error!("Can not get current exe path.err:{:?}",err);
                    continue;
                }
                let target_file = target_file.unwrap();
                writer = BufWriter::new(target_file);
            }

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
            let mut lines = vec![];
            for metric_history in metric_history_list {
                match serde_json::to_string(&metric_history) {
                    Ok(json_v) => {
                        lines.push(json_v);
                    }
                    Err(e) => {
                        warn!("Convert Vec<MetricHistory> to json fail:{:?}",e);
                    }
                };
            }

            if lines.len() < 1{
                continue;
            }

            let lines_str = lines.join("\n");
            info!("v:{:?}",lines_str);
            let lines_str = lines_str.as_bytes();
            match writer.write_all(lines_str).await {
                Ok(_) => {
                    if let Err(e) = writer.flush().await{
                        error!("flush metric history fail.err:{:?}",e);
                    }
                    //success
                    remove_exec_logs(&exec_log_list).await;
                }
                Err(e) => {
                    warn!("write metrics fail:{:?}",e);
                }
            }
        };
    });
}