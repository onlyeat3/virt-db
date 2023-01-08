use actix_settings::{BasicSettings, Settings};
use actix_web::{get, http, web, App, HttpServer, Responder, post, HttpResponse, ResponseError};
use actix_web::web::Json;
use log::info;
use serde_json::json;
use crate::config::app_config::ApplicationSettings;
use crate::model::{CurrentUser, DataWrapper, user_model};
use crate::model::cache_config_model::CacheConfigListReq;
use crate::model::user_model::{GetUserInfoResp, LoginResp};
use crate::utils::jwt::encode_token;

#[post("/cache_config/list")]
pub async fn list(req: web::Json<CacheConfigListReq>, settings: web::Data<BasicSettings<ApplicationSettings>>) -> actix_web::Result<HttpResponse> {

    return Ok(HttpResponse::Ok()
        .json(DataWrapper::<Vec<String>>::success(vec![])));
}