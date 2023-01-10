use std::error;
use std::error::Error;
use std::ops::Add;
use std::rc::Rc;
use std::time::SystemTime;

use crate::config::app_config::SETTINGS;
use crate::error::SysError;
use chrono::{DateTime, Duration, FixedOffset, Local, Utc};
use jsonwebtoken::crypto::verify;
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::jwk::KeyOperations::Verify;
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use log::info;
use serde::{Deserialize, Serialize};

use crate::model::CurrentUser;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    aud: String,
    // sub: String,//token主题
    // iss:String,//被授权人
    subject: CurrentUser,
    exp: i64,
    //token过期时间 UNIX时间戳
    iat: u16, //token创建时间 UNIX时间戳
}

pub fn encode_token(v: CurrentUser, duration_in_seconds: i64) -> anyhow::Result<String> {
    unsafe {
        let exp_date_time = Local::now();
        let exp_date_time = exp_date_time + Duration::seconds(duration_in_seconds);
        info!("token expire at {:?}", exp_date_time);
        let claims = Claims {
            aud: "virt-db-admin".to_string(),
            subject: v,
            exp: exp_date_time.timestamp(),
            iat: 0,
        };
        return anyhow::Ok(encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(SETTINGS.clone().unwrap().application.jwt_secret.as_ref()),
        )?);
    }
}

pub fn parse_current_user(token: String) -> Result<CurrentUser, SysError> {
    Ok(verify_and_parse_token(token)?.claims.subject)
}

pub fn verify_and_parse_token(token: String) -> Result<TokenData<Claims>, SysError> {
    unsafe {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&["virt-db-admin"]);
        let key = &DecodingKey::from_secret(
            SETTINGS
                .clone()
                .unwrap()
                .clone()
                .application
                .jwt_secret
                .as_ref(),
        );
        let token_data = decode::<Claims>(&token, key, &validation)
            .map_err(|_| SysError::BIZ(String::from("token无效或已过期")))?;
        Ok(token_data)
    }
}
