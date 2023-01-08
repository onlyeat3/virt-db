use actix_settings::{BasicSettings, Settings};
use actix_web::{get, http, web, App, HttpServer, Responder, post, HttpResponse, ResponseError};
use actix_web::web::Json;
use log::info;
use serde_json::json;
use crate::config::app_config::ApplicationSettings;
use crate::model::{CurrentUser, DataWrapper, user_model};
use crate::model::user_model::{GetUserInfoResp, LoginResp};
use crate::utils::jwt::encode_token;

#[post("/login")]
pub async fn login(login_req: web::Json<user_model::LoginReq>, settings: web::Data<BasicSettings<ApplicationSettings>>) -> actix_web::Result<HttpResponse> {
    let user_id = 1;
    let user_name = &login_req.username;
    let current_user = CurrentUser {
        user_id,
        user_name: login_req.username.to_string(),
    };
    let token = encode_token(current_user, 3600);
    let mut login_resp = LoginResp {
        access_token: token?,
        refresh_token: "".to_string(),
        expires: "".to_string(),
        roles: vec![String::from("admin")],
        username: user_name.clone(),
    };

    return Ok(HttpResponse::Ok()
        .json(DataWrapper::success(login_resp)));
}

#[get("/getUserInfo")]
pub async fn get_user_info(current_user: CurrentUser) -> impl Responder {
    info!("current_user:{:?}",current_user);
    let get_user_info_resp = GetUserInfoResp {
        user_id: current_user.user_id.to_string(),
        username: current_user.user_name.to_string(),
        real_name: "admin".to_string(),
        avatar: "https://q1.qlogo.cn/g?b=qq&nk=190848757&s=640".to_string(),
        desc: "manager".to_string(),
        password: "".to_string(),
        token: "".to_string(),
        home_path: "/dashboard/analysis".to_string(),
        roles: vec![String::from("admin")],
    };
    return HttpResponse::Ok()
        .json(DataWrapper::success(get_user_info_resp));
}