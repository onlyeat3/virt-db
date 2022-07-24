#[macro_use]
extern crate log;

use std::io::Write;

use clap::{App, Arg};
use clap::{AppSettings, Parser};
use env_logger::{Builder, Target};
use chrono::Local;

use crate::server::start;

// use crate::server::start;

mod server;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(allow_negative_numbers = true)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
struct Cli {
    #[clap(short, long, action = clap::ArgAction::Count)]
    dev: u8,
}

fn init_logger(dev: u8) {
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "debug");
    env_logger::Builder::from_env(env)
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S:%3f"),
                record.level(),
                record.module_path().unwrap_or("<unnamed>"),
                &record.args()
            )
        })
        .init();
    // if dev == 1 {
    //     let mut builder = Builder::from_default_env();
    //     builder.target(Target::Stdout);
    //     builder.init();
    // } else {
    //
    // };
}

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let cli = Cli::parse();
//     init_logger(cli.dev);
//     start().await
// }

fn main(){
    let cli = Cli::parse();
    init_logger(cli.dev);
    start()
}