#[macro_use]
extern crate log;
extern crate core;
extern crate tokio;

use std::error::Error;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use clap::{App, Arg};
use clap::{AppSettings, Parser};
use env_logger::{Builder, Target};

use crate::server::start;

// use crate::server::start;

mod server;
mod mysql_protocol;
mod conv;
mod serde;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(allow_negative_numbers = true)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
struct Cli {
    #[clap(short, long, action = clap::ArgAction::Count)]
    dev: u8,
}

fn init_logger(dev: u8) {
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info");
    env_logger::Builder::from_env(env)
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {} [{}:{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S:%3f"),
                record.level(),
                record.file().unwrap_or("unknown_file"),
                record.line().unwrap_or(0),
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_metrics();
    let cli = Cli::parse();
    init_logger(cli.dev);
    let r = start();
    println!("MySQL Server Proxy Started.");
    return r.await;
}

// fn main(){
//     enable_metrics();
// }

use metrics::{
    decrement_gauge, describe_counter, describe_histogram, gauge, histogram, increment_counter,
    increment_gauge,
};
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_util::MetricKindMask;

use quanta::Clock;
use rand::{thread_rng, Rng};

pub fn enable_metrics(){
    // tracing_subscriber::fmt::init();

    let builder = PrometheusBuilder::new();
    let addr = SocketAddrV4::new(Ipv4Addr::new(127,0,0,1),19091);
    builder
        .with_http_listener(addr)
        .idle_timeout(
            MetricKindMask::COUNTER | MetricKindMask::HISTOGRAM,
            Some(Duration::from_secs(10)),
        )
        .install()
        .expect("failed to install Prometheus recorder");

    // We register these metrics, which gives us a chance to specify a description for them.  The
    // Prometheus exporter records this description and adds it as HELP text when the endpoint is
    // scraped.
    //
    // Registering metrics ahead of using them is not required, but is the only way to specify the
    // description of a metric.
    describe_counter!("tcp_server_loops", "The iterations of the TCP server event loop so far.");
    describe_histogram!(
        "tcp_server_loop_delta_secs",
        "The time taken for iterations of the TCP server event loop."
    );

    // let clock = Clock::new();
    // let mut last = None;

    increment_counter!("idle_metric");
    gauge!("testing", 42.0);

    // Loop over and over, pretending to do some work.
    // loop {
    //     increment_counter!("tcp_server_loops", "system" => "foo");
    //
    //     if let Some(t) = last {
    //         let delta: Duration = clock.now() - t;
    //         histogram!("tcp_server_loop_delta_secs", delta, "system" => "foo");
    //     }
    //
    //     let increment_gauge = thread_rng().gen_bool(0.75);
    //     if increment_gauge {
    //         increment_gauge!("lucky_iterations", 1.0);
    //     } else {
    //         decrement_gauge!("lucky_iterations", 1.0);
    //     }
    //
    //     last = Some(clock.now());
    //
    //     thread::sleep(Duration::from_millis(750));
    // }

}