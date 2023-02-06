#![allow(unused_variables)]
use std::env;
use std::io::Write;
use std::path::Path;

use chrono::Local;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::append::rolling_file::{policy, RollingFileAppender};
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::Config;
use log4rs::config::{Appender, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;
use log::LevelFilter;

const SIZE_MB: u64 = 1024 * 1024;

pub fn init_logger() -> anyhow::Result<()> {
    let stdout = ConsoleAppender::builder().build();

    let file_count = 30;
    let file_size = SIZE_MB * 512;//512MB
    let exec_file_path = env::current_exe()?;
    let log_path = exec_file_path.parent()
        .unwrap()
        .join("logs");
    let log_pattern = log_path.join("server.log");

    #[cfg(feature = "gzip")]
        let roll_pattern = format!("{}/{}", log_path.to_string_lossy(), "log.{}.gz");
    #[cfg(not(feature = "gzip"))]
        let roll_pattern = format!("{}/{}", log_path.to_string_lossy(), "log.{}");

    let trigger = policy::compound::trigger::size::SizeTrigger::new(file_size);
    let roller = policy::compound::roll::fixed_window::FixedWindowRoller::builder()
        .build(&roll_pattern, file_count)
        .unwrap();
    let policy = policy::compound::CompoundPolicy::new(Box::new(trigger), Box::new(roller));

    let file_appender = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S.%3f %Z)} {l} [{t} - {T}] {m}{n}",
        )))
        .build(&log_pattern, Box::new(policy))
        .unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("file", Box::new(file_appender)))
        // .logger(Logger::builder()
        //     .appender("file")
        //     .additive(false)
        //     .build("app::requests", LevelFilter::Info))
        .build(Root::builder().appender("stdout").appender("file").build(LevelFilter::Trace))
        .unwrap();

    let handle = log4rs::init_config(config).unwrap();
    anyhow::Ok(())
}
