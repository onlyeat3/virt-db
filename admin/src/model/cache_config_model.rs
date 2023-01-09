use serde::{Deserialize, Serialize};

use crate::model::{PageParam};

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
