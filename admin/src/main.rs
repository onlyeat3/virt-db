#![allow(unused_imports)]

use std::collections::HashMap;
use crate::config::app_config::ApplicationSettings;
use crate::controller::{cache_config_controller, metric_history_controller, vt_node_controller};
use actix_cors::Cors;
use actix_settings::{ApplySettings as _, BasicSettings};
use actix_web::http::header;
use actix_web::middleware::{Compress, Condition, Logger};
use actix_web::{get, http, post, web, App, HttpServer, Responder};
use log::info;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use serde::{de, Deserialize};
use std::env;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use actix_web::web::Data;
use chrono::{DateTime, Local};
use tokio::sync::Mutex;
use crate::job::vt_node_job::enable_vt_node_alive_check;

mod config;
mod controller;
mod entity;
mod error;
mod model;
mod utils;
mod job;
mod sys_log;

#[derive(Debug, Clone)]
pub struct AppState {
    pub conn: DatabaseConnection,
    pub vt_nodes_lock:Arc<Mutex<HashMap<String, DateTime<Local>>>>
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    sys_log::init_logger();
    let settings: BasicSettings<ApplicationSettings> = config::app_config::load_config();
    env::set_var("DATABASE_URL", settings.application.mysql_url.as_str());
    let mut opt = ConnectOptions::new(settings.application.mysql_url.as_str().to_owned());
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Info); // Setting default PostgreSQL schema

    let conn = Database::connect(opt).await.unwrap();
    let locked_vt_nodes:Arc<Mutex<HashMap<String, DateTime<Local>>>> = Arc::new(Mutex::new(HashMap::new()));
    let app_state = AppState { conn,vt_nodes_lock:locked_vt_nodes };

    enable_vt_node_alive_check(app_state.clone()).await;

    let http_server = HttpServer::new({
        let settings = settings.clone();
        move || {
            //vt-server节点，存活的节点保存在内存
            App::new()
                .wrap(Logger::default())
                .wrap(Condition::new(
                    settings.actix.enable_compression,
                    Compress::default(),
                ))
                .app_data(web::Data::new(settings.clone()))
                .app_data(web::Data::new(app_state.clone()))
                .wrap(
                    Cors::default()
                        .allowed_origin("http://localhost:8848")
                        // .allow_any_origin()
                        .allow_any_method()
                        .allow_any_header()
                        .supports_credentials()
                        .max_age(3600),
                )
                .service(controller::index_controller::index)
                .service(controller::user_controller::login)
                .service(controller::user_controller::update_password)
                .service(controller::user_controller::get_user_info)
                .service(controller::mock_controller::get_async_routes)
                .service(cache_config_controller::list)
                .service(cache_config_controller::create)
                .service(cache_config_controller::delete)
                .service(vt_node_controller::register)
                .service(metric_history_controller::list_sql)
        }
    })
    .apply_settings(&settings)
    .run();
    info!(
        "Server Start at http://127.0.0.1:{}",
        settings.actix.hosts[0].port
    );
    http_server.await
}
