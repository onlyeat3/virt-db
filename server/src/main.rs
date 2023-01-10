#![allow(unused_imports,dead_code)]
#[macro_use]
extern crate log;
extern crate core;
extern crate tokio;

use clap::{AppSettings, Parser};
use log::{error, info};
use std::error::Error;

use crate::server::start;

mod meta;
mod metrics;
mod mysql_protocol;
mod serde;
mod server;
mod sys_config;
mod sys_log;
mod utils;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(allow_negative_numbers = true)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
struct CliArgs {
    #[clap(short, long, default_value = "./config.toml")]
    config_file: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli_args = CliArgs::parse();
    sys_log::init_logger();

    let sys_config_wrapper = sys_config::parse_config(&cli_args.config_file).await;
    if let Err(err) = sys_config_wrapper {
        error!("Read config file fail:{:?}", err);
        return Err(Box::try_from(err).unwrap());
    }
    let sys_config = sys_config_wrapper.unwrap();

    metrics::enable_metrics(sys_config.clone());
    meta::enable_meta_refresh_job(sys_config.clone()).await;

    let r = start(sys_config.clone());
    info!("virt-db Starting at 0.0.0.0:{}",sys_config.server.port);
    return r.await;
}
