use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetricHistory {
    pub sql_str: String,
    pub db_server_port: String,
    pub database_name: String,
    pub avg_duration: i32,
    pub max_duration: i32,
    pub min_duration: i32,
    pub exec_count: i32,
    pub cache_hit_count: i32,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VtNodeRegisterParam {
    pub port: String,
    pub metric_history_list: Vec<MetricHistory>,
}