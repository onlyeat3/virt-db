//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.6

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq,Serialize,Deserialize)]
#[sea_orm(table_name = "cache_config")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub sql_template: String,
    pub duration: i32,
    pub cache_name: String,
    pub remark: String,
    pub enabled: i32,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub created_by: i64,
    pub updated_by: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
