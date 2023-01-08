use actix_web::{get, HttpResponse, post, Responder, web};
use actix_web::web::Data;
use serde_json::json;
use sqlx::{Database, MySql, MySqlPool, Pool, query};

use crate::error::SysError;
use crate::model::{cache_config_model, CurrentUser, DataWrapper, PageResponse};
use crate::model::cache_config_model::{CacheConfigEntity, CacheConfigListParam};

#[post("/cache_config/list")]
pub async fn list(req:web::Json<CacheConfigListParam>,pool: Data<MySqlPool>,current_user: CurrentUser) -> Result<HttpResponse, SysError> {
    let cache_config_list_result = sqlx::query_as::<_, CacheConfigEntity>("SELECT * from cache_config")
        .fetch_all(pool.get_ref())
        .await
        .map_err(anyhow::Error::new)?;

    let data_wrapper = DataWrapper::success(PageResponse{
        list: cache_config_list_result,
        total: 0,
    });
    Ok(HttpResponse::Ok()
        .json(data_wrapper))
}