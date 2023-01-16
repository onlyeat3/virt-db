use sea_orm::prelude::Decimal;
use crate::model::PageParam;
use serde::{Serialize,Deserialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricQueryParam{
    pub page_param: PageParam,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricResult{
    pub sql_str:String,
    pub dates:Vec<String>,
    pub avg_durations:Vec<Decimal>,
    pub min_durations:Vec<i64>,
    pub max_durations:Vec<i64>,
    pub exec_counts: Vec<Decimal>,
    pub cache_hit_counts: Vec<Decimal>,
}