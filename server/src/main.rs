#[macro_use]
extern crate log;
extern crate core;
extern crate tokio;

use std::error::Error;

use clap::{AppSettings, Parser};

use crate::server::start;

// use crate::server::start;

mod mysql_protocol;
mod serde;
mod server;
mod metrics;
mod sys_log;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(allow_negative_numbers = true)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
struct Cli {
    #[clap(short, long, action = clap::ArgAction::Count)]
    dev: u8,
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    metrics::enable_metrics();
    sys_log::init_logger(cli.dev);
    let r = start();
    info!("MySQL Server Proxy Started.");
    return r.await;
}

