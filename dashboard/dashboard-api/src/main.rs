use std::collections::HashMap;
use actix_files as fs;

use actix_web::{
    body::BoxBody,
    dev::ServiceResponse,
    error,
    http::{header::ContentType, StatusCode},
    middleware::{self, ErrorHandlerResponse, ErrorHandlers},
    web, App, Error, HttpResponse, HttpServer, Result,
};

async fn index(
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().body("hello"))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    println!("Listening on: 127.0.0.1:8080, open browser and visit have a try!");
    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default()) // enable logger
            .service(actix_files::Files::new("/public", "dashboard/public").show_files_listing().use_last_modified(true))
            .service(web::resource("/").route(web::get().to(index)))
            .service(web::scope("").wrap(error_handlers()))
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}

// Custom error handlers, to return HTML responses when an error occurs.
fn error_handlers() -> ErrorHandlers<BoxBody> {
    ErrorHandlers::new().handler(StatusCode::NOT_FOUND, not_found)
}


// Error handler for a 404 Page not found error.
fn not_found<B>(res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<BoxBody>> {
    let response = get_error_response(&res, "Page not found");
    Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
        res.into_parts().0,
        response.map_into_left_body(),
    )))
}

// Generic error handler.
fn get_error_response<B>(res: &ServiceResponse<B>, error: &str) -> HttpResponse {
    let request = res.request();

    // Provide a fallback to a simple plain text response in case an error occurs during the
    // rendering of the error page.
    let fallback = |e: &str| {
        HttpResponse::build(res.status())
            .content_type(ContentType::plaintext())
            .body(e.to_string())
    };
    fallback(error)
}