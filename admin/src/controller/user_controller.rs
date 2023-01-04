use actix_web::{get, http, web, App, HttpServer, Responder, post, HttpResponse};
use actix_web::web::Json;
use log::info;
use serde_json::json;
use crate::model::user_model;
use crate::model::user_model::{LoginResp, Role};

#[post("/basic-api/login")]
pub async fn login(login_req: web::Json<user_model::LoginReq>) -> impl Responder {
    let login_resp = LoginResp {
        roles: vec![Role { role_name: String::from("Super Admin"), value: String::from("super") }],
        user_id: "1".to_string(),
        username: login_req.username.to_string(),
        real_name: login_req.username.to_string(),
        desc: "".to_string(),
        token: "".to_string(),
    };
    return HttpResponse::Ok()
        .json(login_resp);
}

#[post("/basic-api/getUserInfo")]
pub async fn get_user_info() -> impl Responder {
    "111111"
}