use actix_web::{get, http, web, App, HttpServer, Responder, post};
use log::info;

#[get("/")]
pub async fn index() -> impl Responder {
    "Are You OK?"
}