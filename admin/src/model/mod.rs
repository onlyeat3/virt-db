use std::error;
use std::error::Error;
use std::future::{Future, Ready};
use std::pin::Pin;

use actix_web::{App, dev, FromRequest, HttpRequest, web};
use actix_web::dev::Payload;
use actix_web::error::ErrorBadRequest;
use actix_web::error::ErrorUnauthorized;
use actix_web::http::header::{HeaderValue, ToStrError};
use futures::future::{err, ok};
use log::info;
use serde::{Deserialize, Serialize};


use crate::utils;

pub mod user_model;
pub mod cache_config_model;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataWrapper<V> {
    pub code: i32,
    pub message: String,
    pub success: bool,
    pub data: Option<V>,
}

impl<V> DataWrapper<V> {
    pub fn success(v: V) -> DataWrapper<V> {
        DataWrapper::result(0, String::from("OK"), v)
    }

    pub fn result(code: i32, message: String, v: V) -> DataWrapper<V> {
        DataWrapper {
            code,
            message,
            success: code == 0,
            data: Some(v),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdParam {
    pub id: i32,
}

impl IdParam {
    pub fn new(id:i32) -> IdParam {
        IdParam{
            id,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResponse<T> {
    pub list: Vec<T>,
    pub total: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageParam{
    pub page_no:Option<i64>,
    pub page_size:Option<i64>,
}

impl PageParam{
    pub fn get_page_no(self) -> u64 {
        let mut page_no = self.page_no.unwrap_or(0) as u64;
        if page_no > 0{
            page_no = page_no - 1
        }
        page_no
    }
    pub fn get_start_row(self) -> u64 {
        let page_no = self.clone().get_page_no();
        let page_size = self.clone().get_limit();
        ((page_no * page_size) - (page_size - 1)) as u64
    }

    pub fn get_limit(self) -> u64 {
        self.page_size.unwrap_or(10) as u64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentUser {
    pub user_id: i32,
    pub user_name: String,
}

impl FromRequest for CurrentUser {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;


    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let req = req.clone();
        return Box::pin(async move {
            return match req.headers().get("Authorization") {
                None => {
                    Err(ErrorUnauthorized("unauthorized"))
                }
                Some(header) => {
                    match header.to_str().ok() {
                        Some(token) => {
                            let r = utils::jwt::parse_current_user(token.replace("Bearer","").trim().to_string())?;
                            Ok(r)
                        }
                        None => {
                            Err(ErrorUnauthorized("unauthorized"))
                        }
                    }
                }
            };
        });
    }
}