use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheConfigListReq {
    pub page_no: i64,
    pub page_size: i64,
    #[serde(rename = "cache_name")]
    pub cache_name: Value,
    #[serde(rename = "sql_template")]
    pub sql_template: Value,
    pub enabled: Value,
}
