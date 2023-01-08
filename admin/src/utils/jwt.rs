use std::error;
use std::error::Error;
use std::ops::Add;
use std::rc::Rc;
use std::time::SystemTime;

use chrono::{DateTime, FixedOffset, Local, Utc};
use jsonwebtoken::{Algorithm, decode, DecodingKey, encode, EncodingKey, Header, TokenData, Validation};
use jsonwebtoken::crypto::verify;
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::jwk::KeyOperations::Verify;
use log::{info};
use serde::{Deserialize, Serialize};
use crate::config::app_config::SETTINGS;

use crate::model::CurrentUser;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    aud: String,
    // sub: String,//token主题
    // iss:String,//被授权人
    subject: CurrentUser,
    exp: i64,//token过期时间 UNIX时间戳
    iat:u16,//token创建时间 UNIX时间戳
}


pub fn encode_token(v:CurrentUser, duration_in_seconds:i32) -> Result<String, Box<dyn error::Error>> {
    unsafe {
        let exp_date_time = Local::now();
        let one_hour = FixedOffset::east_opt(duration_in_seconds)
            .ok_or_else(||{format!("Convert seconds {:?} to FixedOffset fail",duration_in_seconds)})?;
        let exp_date_time= exp_date_time.add(one_hour);
        let exp = exp_date_time.timestamp();
        info!("token expire at {:?}",exp_date_time);
        let claims = Claims{
            aud: "virt-db-admin".to_string(),
            subject: v,
            exp: exp_date_time.timestamp(),
            iat: 0,
        };
        return Ok(encode(&Header::default(), &claims, &EncodingKey::from_secret(SETTINGS.clone().unwrap().application.jwt_secret.as_ref()))?);
    }
}

pub fn parse_current_user(token:String) -> Result<CurrentUser, Box<dyn error::Error>> {
    Ok(verify_and_parse_token(token)?.claims.subject)
}

pub fn verify_and_parse_token(token:String) -> Result<TokenData<Claims>, Box<dyn error::Error>> {
    unsafe {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&["virt-db-admin"]);
        let token_data = decode::<Claims>(&token, &DecodingKey::from_secret(SETTINGS.clone().unwrap().clone().application.jwt_secret.as_ref()), &validation)
            .map_err(|err|{err.to_string()})?;
        Ok(token_data)
    }
}