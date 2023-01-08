use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::model::{PageParam};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CacheConfigEntity {
    pub id: Option<i32>,
    pub sql_template: Option<String>,
    pub duration: Option<i32>,
    pub cache_name: Option<String>,
    pub enabled: Option<i32>,
    pub created_by: Option<i32>,
    pub updated_by: Option<i32>,
}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheConfigListParam {
    pub page_param: PageParam,
    #[serde(rename = "cache_name")]
    pub cache_name: Option<String>,
    #[serde(rename = "sql_template")]
    pub sql_template: Option<String>,
    pub enabled: Option<i32>,
}
