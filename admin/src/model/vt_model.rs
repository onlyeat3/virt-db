use serde::{Serialize,Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct MetricHistory {
    pub sql_str: String,
    db_server_port: String,
    database_name: String,
    avg_duration: i32,
    max_duration: i64,
    exec_count: usize,
    cache_hit_count: i32,
    created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VtNodeRegisterParam{
    pub port:String,
    pub metric_history_list:Vec<MetricHistory>,
}