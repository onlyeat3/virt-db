#![allow(unused_imports, dead_code)]
extern crate byteorder;
extern crate core;
#[macro_use]
extern crate log;
extern crate tokio;

use std::error::Error;

use clap::{AppSettings, Parser};
use log::{error};
use tokio::sync::mpsc;

use crate::server::start;
use crate::sys_assistant_client::{enable_cache_task_handle_job, enable_metric_writing_job};

mod meta;
mod sys_assistant_client;
mod server;
mod sys_config;
mod sys_log;
mod utils;
mod math;
mod protocol;
mod sys_error;
mod sys_redis;
mod serve;

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
    sys_log::init_logger()?;

    let sys_config_wrapper = sys_config::parse_config(&cli_args.config_file);
    if let Err(err) = sys_config_wrapper {
        error!("Read config file fail:{:?}", err);
        return Err(Box::try_from(err).unwrap());
    }
    let sys_config = sys_config_wrapper.unwrap();
    let virt_db_config = sys_config.clone();

    let (exec_log_channel_sender, exec_log_channel_receiver) = mpsc::channel(10*100_000);
    let (cache_load_task_channel_sender, cache_load_task_channel_receiver) = mpsc::channel(10*100_000);

    enable_metric_writing_job(sys_config.clone(), exec_log_channel_receiver);
    meta::enable_meta_refresh_job(sys_config.clone());
    enable_cache_task_handle_job(sys_config.clone(),cache_load_task_channel_receiver);

    start(virt_db_config, exec_log_channel_sender,cache_load_task_channel_sender).await.unwrap();
    Ok(())
}
