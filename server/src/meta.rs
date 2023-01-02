use std::thread;
use std::thread::Thread;
use std::error::Error;
use std::time::{Duration, Instant};
use chrono::DateTime;
use tokio::spawn;
use crate::sys_config::VirtDBConfig;
use log::{info};
use mysql_async::{Conn, Opts, QueryResult, TextProtocol};
use mysql_async::prelude::Queryable;
use sqlx::mysql::MySqlPoolOptions;

static mut CACHE_CONFIG_ENTITY_LIST: Vec<CacheConfigEntity> = vec![];

fn get_cache_config_entity_list() -> &'static Vec<CacheConfigEntity> {
    unsafe {
        &CACHE_CONFIG_ENTITY_LIST
    }
}

fn set_cache_config_entity_list(entity_list: Vec<CacheConfigEntity>) {
    unsafe {
        CACHE_CONFIG_ENTITY_LIST.clear();
        for entity in entity_list {
            CACHE_CONFIG_ENTITY_LIST.push(entity);
        }
    }
}

#[derive(sqlx::FromRow, Debug)]
pub struct CacheConfigEntity {
    id: i32,
    sql_template: String,
    duration: i32,
    cache_name: String,
    remark: String,
    enabled: i32,
    // created_at: chrono::DateTime<chrono::Utc>,
    // updated_at: chrono::DateTime<chrono::Utc>,
    created_by: i64,
    updated_by: i64,
}

pub async fn enable_meta_refresh_job(sys_config: VirtDBConfig) {
    let meta_config = sys_config.meta_db.clone();
    tokio::spawn(async move {
        let meta_mysql_username = meta_config.username;
        let meta_mysql_password = meta_config.password;
        let meta_mysql_ip = meta_config.ip;
        let meta_mysql_port = meta_config.port;
        let meta_mysql_database = meta_config.database;
        let mysql_url = format!("mysql://{}:{}@{}:{}/{}", meta_mysql_username, meta_mysql_password, meta_mysql_ip, meta_mysql_port, meta_mysql_database).clone();
        let pool = MySqlPoolOptions::new()
            .max_connections(1)
            .connect(&mysql_url).await.unwrap();
        let sql = "select * from cache_config where enabled = true";
        loop {
            let cache_config_list = sqlx::query_as::<_, CacheConfigEntity>(sql)
                .fetch_all(&pool)
                .await.unwrap();
            set_cache_config_entity_list(cache_config_list);
            // info!("cache_config_list:{:?}", get_cache_config_entity_list());


            thread::sleep(Duration::from_secs(10));
        }
    });
}