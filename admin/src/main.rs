use actix_cors::Cors;
use actix_settings::{Settings};
use actix_web::{App, HttpServer};
use actix_web::http::header;
use actix_web::middleware::Logger;
use env_logger::Env;
use log::info;

mod controller;
mod model;

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    let settings = Settings::parse_toml("actix.toml")
        .expect("Failed to parse `Settings` from config.toml");
    info!("settings:{:?}",settings);

    let port = 8080;
    let http_server = HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .wrap( Cors::default()
                       .allowed_origin("http://localhost:8080")
                       .allowed_origin("http://localhost:3100")
                       .allowed_methods(vec!["GET", "POST"])
                       .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
                       .allowed_header(header::CONTENT_TYPE)
                       .supports_credentials()
                       .max_age(3600),
            )
            .service(controller::index_controller::index)
            .service(controller::user_controller::login)
            .service(controller::user_controller::get_user_info)
    })
        .bind(("0.0.0.0", port))?
        .run();
    info!("Server Start at http://127.0.0.1:{}", port);
    http_server.await
}
