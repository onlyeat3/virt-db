#[macro_use]
use std::error::Error;
use std::path::{Path, PathBuf};
use tokio::fs;
use toml;
use serde_derive::Deserialize;
use log::{debug, error, info, trace};

#[derive(Debug, Deserialize, Clone)]
pub struct VirtDBConfig {
    pub server: ServerConfig,
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
    //客户端连接账号
    pub username: String,
    //客户端链接密码
    pub password: String,
}

/**
 * 被代理的数据库信息
 */
#[derive(Debug, Deserialize, Clone)]
pub struct BackendMySQLServerConfig {
    pub ip: String,
    pub port: i32,
    pub username: String,
    pub password: String,
}

/**
 * 缓存用的Redis信息
 */
#[derive(Debug, Deserialize, Clone)]
pub struct RedisServerConfig {
    pub ip: String,
    pub port: i32,
    pub requirepass: String,
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

pub async fn parse_config(config_file: &str) -> Result<VirtDBConfig, std::io::Error> {
    let mut base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let real_config_file = if Path::new(config_file).exists() {
        config_file
    } else {
        base_dir.push("config.toml");
        base_dir.to_str().unwrap()
    };
    let real_config_file_path = Path::new(real_config_file);
    info!("Try read config from file:{:?}",real_config_file_path);
    if !real_config_file_path.exists() {
        error!("Config file:{:?} Not Exists",real_config_file_path.to_path_buf());
    }
    let toml_str = fs::read_to_string(real_config_file).await?;
    let virt_db_config: VirtDBConfig = toml::from_str(toml_str.as_str()).unwrap();
    return Ok(virt_db_config);
}