#![allow(unused_imports, dead_code)]

use actix_settings::{BasicSettings, Settings};
use actix_web::{App, get, http, HttpResponse, HttpServer, post, Responder, ResponseError, web};
use actix_web::web::{Data, Json};
use futures::FutureExt;
use log::info;
use sea_orm::{DbErr, EntityTrait, QueryFilter};
use sea_orm::ActiveValue::Set;
use serde_json::json;
use md5;
use rand::distributions::Alphanumeric;
use rand::prelude::*;
use sea_orm::sea_query::Expr;

use crate::AppState;
use crate::config::app_config::ApplicationSettings;
use crate::entity::prelude::SysUser;
use crate::entity::sys_user;
use crate::entity::sys_user::{Column, Model};
use crate::error::SysError;
use crate::model::{CurrentUser, DataWrapper, user_model};
use crate::model::user_model::{GetUserInfoResp, LoginResp, PasswordUpdateParam};
use crate::utils::jwt::encode_token;
use crate::utils::password;

#[post("/login")]
pub async fn login(
    login_req: web::Json<user_model::LoginReq>,
    _: web::Data<BasicSettings<ApplicationSettings>>,
    app_state:Data<AppState>
) -> Result<impl Responder,SysError> {
    let sys_user = SysUser::find()
        .filter(Expr::col(Column::UserName).eq(login_req.username.clone()))
        .filter(Expr::col(Column::UserStatus).eq(1))
        .one(&app_state.conn)
        .await
        .map_err(anyhow::Error::new)?
        .ok_or(SysError::BIZ("用户不存在或被禁用".to_string()))?;
    if sys_user.user_password != password::encode(login_req.password.clone(),sys_user.user_salt)?{
        return Err(SysError::BIZ("用户名或密码错误".to_string()));
    }
    let user_id = sys_user.id;
    let user_name = &login_req.username;
    let current_user = CurrentUser {
        user_id,
        user_name: login_req.username.to_string(),
    };
    let token = encode_token(current_user, 3600 * 24);
    let login_resp = LoginResp {
        access_token: token?,
        refresh_token: "".to_string(),
        expires: "".to_string(),
        roles: vec![String::from("admin")],
        username: user_name.clone(),
    };

    return Ok(DataWrapper::<LoginResp>::success(login_resp));
}

#[get("/getUserInfo")]
pub async fn get_user_info(current_user: CurrentUser) -> impl Responder {
    info!("current_user:{:?}", current_user);
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
    return HttpResponse::Ok().json(DataWrapper::success(get_user_info_resp));
}

#[post("/update_password")]
pub async fn update_password(password_update_param: Json<PasswordUpdateParam>, current_user: CurrentUser, app_state: Data<AppState>) -> Result<impl Responder, SysError> {
    let min_password_len = 8;
    if password_update_param.new_password.len() < min_password_len{
        return Err(SysError::BIZ(format!("密码长度过短，至少需要{}个字符",min_password_len)));
    }
    let sys_user_result = SysUser::find_by_id(current_user.user_id as i64)
        .one(&app_state.conn)
        .await
        .map_err(anyhow::Error::new)?;
    let sys_user = match sys_user_result {
        None => Err(SysError::BIZ("无效用户".to_string())),
        Some(sys_user) => Ok(sys_user)
    }?;

    if password_update_param.new_password != password_update_param.new_password_confirm {
        return Err(SysError::BIZ("新密码和确认密码不一致".to_string()));
    }
    if password::encode(password_update_param.current_password.clone(),sys_user.user_salt)? != sys_user.user_password {
        return Err(SysError::BIZ("旧密码错误".to_string()));
    }
    let salt: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();
    let encoded_password = password::encode(password_update_param.new_password.clone(),salt)?;

    let sys_user_model = sys_user::ActiveModel {
        id: Set(sys_user.id),
        user_password: Set(encoded_password),
        ..Default::default()
    };
    let _ = SysUser::update(sys_user_model)
        .exec(&app_state.conn)
        .await
        .map_err(anyhow::Error::new)?;

    return Ok(DataWrapper::success("".to_string()));
}
