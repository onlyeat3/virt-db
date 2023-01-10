use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Add;

use actix_web::cookie::time::Error::Format;
use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use log::error;
use sea_orm::strum::Display;

use crate::model::DataWrapper;

#[derive(Debug, Display)]
pub enum SysError {
    BIZ(String),
    SYSTEM(anyhow::Error),
}

impl Error for SysError {}

impl actix_web::error::ResponseError for SysError {
    fn status_code(&self) -> StatusCode {
        StatusCode::OK
    }

    fn error_response(&self) -> HttpResponse {
        let error_msg = match self {
            SysError::BIZ(msg) => msg,
            SysError::SYSTEM(err) => {
                error!("Unknown System Error:{:?}", err);
                "未知错误"
            }
        };
        let response_body = String::from("{")
            .add("\"code\": -1,")
            .add("\"message\": \"")
            .add(error_msg)
            .add("\",")
            .add("\"success\": false,")
            .add("\"data\": null")
            .add("}")
            .to_string();

        HttpResponse::build(self.status_code())
            .insert_header(ContentType::json())
            .body(response_body.to_owned())
    }
}

impl From<anyhow::Error> for SysError {
    fn from(err: anyhow::Error) -> SysError {
        SysError::SYSTEM(err)
    }
}
