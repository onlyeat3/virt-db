use std::env;
use std::time::Duration;
use actix_cors::Cors;
use actix_settings::{ApplySettings as _, BasicSettings};
use actix_web::{App, get, http, HttpServer, post, Responder, web};
use actix_web::http::header;
use actix_web::middleware::{Compress, Condition, Logger};
use env_logger::Env;
use log::info;
use sea_orm::{ConnectOptions, Database};
use serde::{de, Deserialize};
use sqlx::MySqlPool;
use crate::config::app_config::ApplicationSettings;
use crate::controller::cache_config_controller;

mod controller;
mod model;
mod utils;
mod config;
mod error;
mod entity;


#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    // std::env::set_var("RUST_LOG", "info");
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    let settings: BasicSettings<ApplicationSettings> = config::app_config::load_config();
    env::set_var("DATABASE_URL",settings.application.mysql_url.as_str());
    let mut opt = ConnectOptions::new(settings.application.mysql_url.as_str().to_owned());
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Info); // Setting default PostgreSQL schema

    let db = Database::connect(opt).await?;

    let http_server = HttpServer::new({
        let settings = settings.clone();
        move || {
            App::new()
                .wrap(Logger::default())
                .wrap(Logger::new("%a %{User-Agent}i"))

                .wrap(Condition::new(
                    settings.actix.enable_compression,
                    Compress::default(),
                ))
                .app_data(web::Data::new(settings.clone()))
                .app_data(web::Data::new(db))

                .wrap(Cors::default()
                    .allowed_origin("http://localhost:8848")
                    // .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                    .supports_credentials()
                    .max_age(3600)
                )
                .service(controller::index_controller::index)
                .service(controller::user_controller::login)
                .service(controller::user_controller::get_user_info)
                .service(controller::mock_controller::get_async_routes)
                .service(cache_config_controller::list)
        }
    })
        .apply_settings(&settings)
        .run();
    info!("Server Start at http://127.0.0.1:{}", settings.actix.hosts[0].port);
    http_server.await
}
