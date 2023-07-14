#![allow(unused_imports)]
use std::error::Error;
use std::{env, fs};
use log::{debug, error, info, trace};
use serde_derive::Deserialize;
use std::path::{Path, PathBuf};
use toml;

#[derive(Debug, Deserialize, Clone)]
pub struct VirtDBConfig {
    pub server: ServerConfig,
    pub admin: AdminConfig,
    pub metric: MetricConfig,
    pub mysql: BackendMySQLServerConfig,
    pub redis: RedisServerConfig,
    pub meta_db: MetaDbConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    //是否开发模式
    pub dev: bool,
    //服务端口
    pub port: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AdminConfig {
    //是否开发模式
    pub address: String,
}

/**
 * 被代理的数据库信息
 */
#[derive(Debug, Deserialize, Clone)]
pub struct BackendMySQLServerConfig {
    pub ip: String,
    pub port: i32,
}
/**
 * 指标监控配置
 */
#[derive(Debug, Deserialize, Clone)]
pub struct MetricConfig {
    pub expose_port: u16,
}

/**
 * 缓存用的Redis信息
 */
#[derive(Debug, Deserialize, Clone)]
pub struct RedisServerConfig {
    pub nodes: String,
}

/**
 * 服务的源数据
 */
#[derive(Debug, Deserialize, Clone)]
pub struct MetaDbConfig {
    pub ip: String,
    pub port: i32,
    pub username: String,
    pub password: String,
    pub database: String,
    pub refresh_duration_in_seconds: u64,
}

pub fn parse_config(config_file: &str) -> Result<VirtDBConfig, std::io::Error> {
    let current_exec_path = env::current_exe().expect("Get Workdir fail");
    let mut base_dir = current_exec_path.parent().expect("Get Workdir fail.").to_path_buf();
    let real_config_file = if Path::new(config_file).exists() {
        info!("Read config from cli config [{:?}]",config_file);
        config_file
    } else {
        base_dir.push("config.toml");
        base_dir.to_str().unwrap()
    };
    let real_config_file_path = Path::new(real_config_file);
    info!("Try read config from file:{:?},argument `config_file`:{:?}", real_config_file_path,config_file);
    if !real_config_file_path.exists() {
        error!(
            "Config file:{:?} Not Exists",
            real_config_file_path.to_path_buf()
        );
    }
    let toml_str = fs::read_to_string(real_config_file)?;
    let virt_db_config: VirtDBConfig = toml::from_str(toml_str.as_str()).unwrap();
    return Ok(virt_db_config);
}
