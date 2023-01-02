use chrono::Local;
use std::io::Write;

pub fn init_logger() {
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