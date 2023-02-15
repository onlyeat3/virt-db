use std::{thread, time};
use std::time::Duration;
use mysql::{Conn, Opts, Pool};
use mysql::prelude::*;

use crate::sys_config::VirtDBConfig;

static mut CACHE_CONFIG_ENTITY_LIST: Vec<CacheConfigEntity> = vec![];

pub fn get_cache_config_entity_list() -> &'static Vec<CacheConfigEntity> {
    unsafe { &CACHE_CONFIG_ENTITY_LIST }
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
    pub id: i32,
    pub sql_template: String,
    pub duration: i32,
    pub cache_name: String,
    pub remark: String,
    pub enabled: i32,
    // pub created_at: chrono::DateTime<chrono::Utc>,
    // pub updated_at: chrono::DateTime<chrono::Utc>,
    pub created_by: i64,
    pub updated_by: i64,
}

pub fn enable_meta_refresh_job(sys_config: VirtDBConfig) {
    let meta_config = sys_config.meta_db.clone();
    thread::spawn(move || {
        let meta_mysql_username = meta_config.username;
        let meta_mysql_password = meta_config.password;
        let meta_mysql_ip = meta_config.ip;
        let meta_mysql_port = meta_config.port;
        let meta_mysql_database = meta_config.database;
        let mysql_url = format!(
            "mysql://{}:{}@{}:{}/{}",
            meta_mysql_username,
            meta_mysql_password,
            meta_mysql_ip,
            meta_mysql_port,
            meta_mysql_database
        )
            .clone();
        let pool = Pool::new(&*mysql_url.clone()).unwrap();
        loop {
            let conn_result = pool.get_conn();

            match conn_result {
                Ok(mut conn) => {
                    let cache_config_list =
                        "select id,sql_template,duration from cache_config where enabled = true"
                            .with(())
                            .map(&mut conn, |(id, sql_template, duration)| {
                                CacheConfigEntity {
                                    id,
                                    sql_template,
                                    duration,
                                    cache_name: "".to_string(),
                                    remark: "".to_string(),
                                    enabled: -1,
                                    created_by: -1,
                                    updated_by: -1,
                                }
                            })
                            .unwrap();
                    set_cache_config_entity_list(cache_config_list);
                    debug!("reload cache_config_list finish");
                }
                Err(err) => {
                    warn!("Connect Meta DB fail.err:{:?}",err);
                }
            }
            thread::sleep(Duration::from_secs(meta_config.refresh_duration_in_seconds));
        }
    });
    info!("CacheConfig auto-reload task Running");
}
