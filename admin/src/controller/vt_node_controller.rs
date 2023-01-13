#![allow(unused_imports, dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use actix_settings::{BasicSettings, Settings};
use actix_web::{HttpResponse, post, web};
use actix_web::web::Data;
use anyhow::Error;
use chrono::{DateTime, Duration, Local};
use log::info;

use crate::AppState;
use crate::config::app_config::ApplicationSettings;
use crate::error::SysError;
use crate::model::{DataWrapper, vt_model};

#[post("/vt_node/register")]
pub async fn register(req: web::Json<vt_model::VtNodeRegisterParam>,
                      app_state_data: Data<AppState>, ) -> Result<HttpResponse, SysError> {
    let key = &format!("{}:{}", req.host, req.port);
    let expire_at = Local::now() + Duration::seconds(60*2);
    let vt_nodes_lock = Arc::clone(&app_state_data.vt_nodes_lock);
    let mut vt_nodes = vt_nodes_lock.lock().await;

    vt_nodes.insert(key.clone(), expire_at);
    return Ok(HttpResponse::Ok().json(DataWrapper::success("")));
}