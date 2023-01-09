use sea_orm::{IntoActiveModel, NotSet};
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};

use crate::entity::cache_config;
use crate::entity::cache_config::{ActiveModel, Model};
use crate::model::PageParam;
use crate::utils::orm::option_to_active_value;

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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheConfigCreateParam {
    pub id: Option<i32>,
    #[serde(rename = "cache_name")]
    pub cache_name: Option<String>,
    #[serde(rename = "sql_template")]
    pub sql_template: Option<String>,
    pub remark: Option<String>,
    pub duration: Option<i32>,
    pub enabled: Option<i32>,
}

impl CacheConfigCreateParam{
    pub fn to_active_model(self) -> ActiveModel {
        let mut cache_config_entity = ActiveModel{
            ..Default::default()
        };
        cache_config_entity.id = option_to_active_value(self.id);
        cache_config_entity.sql_template = option_to_active_value(self.sql_template);
        cache_config_entity.duration = option_to_active_value(self.duration);
        cache_config_entity.cache_name = option_to_active_value(self.cache_name);
        cache_config_entity.remark = option_to_active_value(self.remark);
        cache_config_entity.enabled = option_to_active_value(self.enabled);
        return cache_config_entity;
    }
}
