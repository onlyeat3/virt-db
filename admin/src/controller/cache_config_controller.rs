use actix_web::{get, HttpResponse, post, Responder, web};
use actix_web::web::{BufMut, Data};
use sea_orm::{DatabaseConnection, entity, EntityTrait, PaginatorTrait, QueryFilter};
use sea_orm::sea_query::{Expr, Query, SimpleExpr};
use serde_json::json;
use sqlx::{Database, MySql, MySqlPool, Pool, query, QueryBuilder};
use crate::entity::prelude::CacheConfig;

use crate::error::SysError;
use crate::model::{cache_config_model, CurrentUser, DataWrapper, PageResponse};
use crate::model::cache_config_model::{CacheConfigEntity, CacheConfigListParam};

#[post("/cache_config/list")]
pub async fn list(req:web::Json<CacheConfigListParam>, conn: Data<DatabaseConnection>, current_user: CurrentUser) -> Result<HttpResponse, SysError> {
    // let query = CacheConfig::find();
    // if req.cache_name.is_some() && !req.cache_name?.is_empty(){
    //     query.filter(Expr::col(entity::Column::).is_null())
    // }
    // query.paginate(conn.get_ref(), req.page_param.page_size as u64)
    //     .limit(req.page_param.page_size as usize);
    let cache_config_list_result = sqlx::query_as::<_, CacheConfigEntity>("SELECT * from cache_config")
        .fetch_all(conn.get_ref())
        .await
        .map_err(anyhow::Error::new)?;

    let data_wrapper = DataWrapper::success(PageResponse{
        list: cache_config_list_result,
        total: 0,
    });
    Ok(HttpResponse::Ok()
        .json(data_wrapper))
}