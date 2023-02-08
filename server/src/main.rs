#![allow(unused_imports, dead_code)]
#[macro_use]
extern crate log;
extern crate core;
extern crate tokio;
extern crate lazy_static;
extern crate byteorder;
#[macro_use]
extern crate futures;
extern crate mysql_common as myc;
extern crate mysql_common;
#[macro_use]
extern crate tokio_core;

use clap::{AppSettings, Parser};
use log::{error, info};
use std::error::Error;
use tokio::runtime::Builder;

use crate::server::start;
use crate::sys_metrics::enable_node_live_refresh_job;

mod meta;
mod sys_metrics;
mod mysql_protocol;
mod serde;
mod server;
mod sys_config;
mod sys_log;
mod utils;
mod math;
mod protocol;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(allow_negative_numbers = true)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
struct CliArgs {
    #[clap(short, long, default_value = "./config.toml")]
    config_file: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli_args = CliArgs::parse();
    sys_log::init_logger()?;

    let sys_config_wrapper = sys_config::parse_config(&cli_args.config_file);
    if let Err(err) = sys_config_wrapper {
        error!("Read config file fail:{:?}", err);
        return Err(Box::try_from(err).unwrap());
    }
    let sys_config = sys_config_wrapper.unwrap();
    let virt_db_config = sys_config.clone();

    let rt = Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        sys_metrics::enable_metrics(sys_config.clone());
        meta::enable_meta_refresh_job(sys_config.clone()).await;
        enable_node_live_refresh_job(sys_config.clone()).await;
    });

    start(virt_db_config).unwrap();
    Ok(())
}
