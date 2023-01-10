#![allow(unused_imports,unused_variables)]
use crate::entity::cache_config;
use crate::entity::cache_config::{ActiveModel, Entity, Model};
use crate::entity::prelude::CacheConfig;
use crate::AppState;
use actix_web::web::{BufMut, Data};
use actix_web::{get, post, web, HttpResponse, Responder};
use anyhow::Error;
use log::info;
use sea_orm::sea_query::{Expr, Query, SimpleExpr};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    entity, ActiveModelTrait, ActiveValue, DatabaseConnection, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, QuerySelect, Select,
};
use serde_json::json;
use std::borrow::Borrow;
use std::cell::RefCell;
use std::rc::Rc;

use crate::error::SysError;
use crate::model::cache_config_model::{CacheConfigCreateParam, CacheConfigListParam};
use crate::model::{cache_config_model, CurrentUser, DataWrapper, IdParam, PageResponse};

#[post("/cache_config/list")]
async fn list(
    req: web::Json<CacheConfigListParam>,
    app_state: Data<AppState>,
    current_user: CurrentUser,
) -> Result<HttpResponse, SysError> {
    let conn = &app_state.conn;
    let mut query = CacheConfig::find();
    if req.cache_name.is_some() && !req.cache_name.as_ref().unwrap().is_empty() {
        query = query
            .filter(Expr::col(cache_config::Column::CacheName).eq(req.cache_name.clone().unwrap()));
    }
    if req.sql_template.is_some() && !req.sql_template.as_ref().unwrap().is_empty() {
        query = query.filter(
            Expr::col(cache_config::Column::SqlTemplate).eq(req.sql_template.clone().unwrap()),
        );
    }
    if req.enabled.is_some() {
        query = query.filter(Expr::col(cache_config::Column::Enabled).eq(req.enabled.unwrap()));
    }

    let page_param = req.clone().page_param;
    let paginator = query.paginate(conn, page_param.clone().get_limit());
    let items_and_page_number = paginator
        .num_items_and_pages()
        .await
        .map_err(anyhow::Error::new)?;
    let list = paginator
        .fetch_page(page_param.clone().get_page_no())
        .await
        .map_err(anyhow::Error::new)?;
    info!(
        "page_no:{:?},page_size:{:?},list:{:?}",
        page_param.page_no, page_param.page_size, list
    );

    let data_wrapper = DataWrapper::success(PageResponse {
        list,
        total: items_and_page_number.number_of_items as i64,
    });
    Ok(HttpResponse::Ok().json(data_wrapper))
}

#[post("/cache_config/createOrUpdate")]
async fn create(
    req: web::Json<CacheConfigCreateParam>,
    app_state: Data<AppState>,
    current_user: CurrentUser,
) -> Result<HttpResponse, SysError> {
    let conn = &app_state.conn;
    req.to_owned()
        .to_active_model()
        .save(conn)
        .await
        .map_err(Error::new)?;
    let data_wrapper = DataWrapper::success("");
    Ok(HttpResponse::Ok().json(data_wrapper))
}

#[post("/cache_config/delete")]
async fn delete(
    req: web::Json<IdParam>,
    app_state: Data<AppState>,
    current_user: CurrentUser,
) -> Result<HttpResponse, SysError> {
    let conn = &app_state.conn;
    cache_config::Entity::delete_by_id(req.id)
        .exec(conn)
        .await
        .map_err(Error::new)?;
    let data_wrapper = DataWrapper::success("");
    Ok(HttpResponse::Ok().json(data_wrapper))
}
