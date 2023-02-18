#![allow(unused_imports, dead_code)]

use std::collections::HashMap;
use std::fmt::format;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use actix_settings::{BasicSettings, Settings};
use actix_web::{HttpRequest, HttpResponse, post, web};
use actix_web::web::Data;
use anyhow::Error;
use chrono::{DateTime, Duration, Local, NaiveDateTime, TimeZone, Utc};
use log::info;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;

use crate::AppState;
use crate::config::app_config::ApplicationSettings;
use crate::entity::metric_history;
use crate::entity::metric_history::ActiveModel;
use crate::entity::prelude::MetricHistory;
use crate::error::SysError;
use crate::model::{DataWrapper, vt_model};

#[post("/vt_node/register")]
pub async fn register(req_param: web::Json<vt_model::VtNodeRegisterParam>,
                      _req: HttpRequest,
                      app_state_data: Data<AppState>, ) -> Result<HttpResponse, SysError> {
    let remote_addr_option = _req.peer_addr();
    if remote_addr_option.is_none() {
        return Ok(HttpResponse::Ok().json(DataWrapper::result(-1, String::from("无法获取客户端IP"), Some(""))));
    }

    let remote_addr = remote_addr_option.unwrap();
    let remote_ip = format!("{}", remote_addr.ip());

    let key = &format!("{}:{}", remote_ip, req_param.port);
    let expire_at = Local::now() + Duration::seconds(60 * 2);
    let vt_nodes_lock = Arc::clone(&app_state_data.vt_nodes_lock);
    let mut vt_nodes = vt_nodes_lock.lock().await;

    vt_nodes.insert(key.clone(), expire_at);

    for x in &req_param.metric_history_list {
        let x = x.clone();
        let created_at = Local.timestamp_millis_opt(x.created_at*1000).unwrap();
        let _ = ActiveModel {
            id: Default::default(),
            sql_str: Set(x.sql_str),
            db_server_ip: Set(remote_ip.clone()),
            db_server_port: Set(x.db_server_port),
            database_name: Set(x.database_name),
            avg_duration: Set(x.avg_duration as i32),
            max_duration: Set(x.max_duration as i32),
            min_duration: Set(x.min_duration as i32),
            exec_count: Set(x.exec_count as i32),
            cache_hit_count: Set(x.cache_hit_count as i32),
            created_at: Set(created_at.naive_local()),
        }.insert(&app_state_data.conn)
            .await
            .map_err(anyhow::Error::new)?;
    }

    return Ok(HttpResponse::Ok().json(DataWrapper::success("")));
}