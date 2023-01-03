use actix_web::{get, http, web, App, HttpServer, Responder};
use log::info;

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Hello {name}!")
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let port = 8080;
    let http_server = HttpServer::new(|| App::new().service(greet))
        .bind(("0.0.0.0", port))?
        .run();
    info!("Server Start at http://127.0.0.1:{}", port);
    http_server.await
}
