use std::ops::Add;
use actix_web::{get, HttpResponse, post, Responder, web};
use actix_web::web::{Data, Json};
use chrono::{DateTime, Duration, Local, LocalResult, TimeZone, Utc};
use chrono::prelude::*;
use futures::TryFutureExt;
use log::info;
use sea_orm::{DatabaseBackend, DbBackend, JsonValue, PaginatorTrait, Statement};
use sea_orm::{DeriveIden, entity::*, EnumIter, query::*};
use sea_orm::prelude::{DateTimeLocal, Decimal};
use sea_orm::sea_query::Query;

use crate::AppState;
use crate::entity::prelude::MetricHistory;
use crate::error::SysError;
use crate::model::{DataWrapper, metric, PageResponse};
use crate::model::metric::{MetricQueryParam, MetricResult};

#[post("/metric/list_sql")]
pub async fn list_sql(metric_param: Json<MetricQueryParam>, app_state: Data<AppState>) -> Result<HttpResponse, SysError> {
    let conn = &app_state.conn;
    let sql = r#"select sql_str from metric_history group by sql_str"#;
    let page_param = metric_param.page_param.clone();
    let paginator = JsonValue::find_by_statement(Statement::from_sql_and_values(DatabaseBackend::MySql, sql, vec![]))
        .paginate(conn, page_param.clone().get_limit());
    let total = paginator.num_items_and_pages()
        .await
        .map_err(anyhow::Error::new)?
        .number_of_items;
    let list: Vec<JsonValue> = paginator
        .fetch_page(page_param.get_page_no())
        .await
        .map_err(anyhow::Error::new)?;
    let metric_result_vec: Vec<MetricResult> = list
        .iter()
        .map(|x| x.as_object())
        .filter(|x| x.is_some())
        .map(|x| x.unwrap())
        .map(|x| x.get("sql_str"))
        .filter(|x| x.is_some())
        .map(|x| x.unwrap())
        .map(|x| x.as_str())
        .filter(|x| x.is_some())
        .map(|x| x.unwrap())
        .map(|x| {
            MetricResult {
                sql_str: String::from(x),
                dates: vec![],
                avg_durations: vec![],
                min_durations: vec![],
                max_durations: vec![],
                exec_counts: vec![],
                cache_hit_counts: vec![],
            }
        })
        .collect();
    let mut final_metric_result_vec: Vec<MetricResult> = vec![];
    for mut x in metric_result_vec.clone() {
        let _ = conn.execute(Statement::from_sql_and_values(DbBackend::MySql,"set time_zone = '+08:00';",vec![]))
            .await
            .unwrap();
        let sub_sql = format!(r#"
    SELECT
      concat(date,"") as date,
      ifnull(avg_duration, 0) AS avg_duration,
      ifnull(max_duration, 0) AS max_duration,
      ifnull(min_duration, 0) AS min_duration,
      ifnull(exec_count, 0) AS exec_count,
      ifnull(cache_hit_count, 0) AS cache_hit_count
    FROM
      (
        SELECT
          date_format(
            DATE_SUB(now(), INTERVAL t.help_topic_id MINUTE),
            '%H:%i:00'
          ) AS 'date'
        FROM
          mysql.help_topic t
        WHERE
          t.help_topic_id <= timestampdiff(
            minute,
            DATE_SUB(now(), INTERVAL 30 MINUTE),
            now()
          )
      ) t1
      LEFT JOIN (
        select
          avg(avg_duration) as avg_duration,
          max(max_duration) as max_duration,
          min(min_duration) as min_duration,
          sum(exec_count) as exec_count,
          sum(cache_hit_count) as cache_hit_count,
          created_at
        from
          (
            select
              id,
              sql_str,
              avg_duration,
              max_duration,
              min_duration,
              exec_count,
              cache_hit_count,
              DATE_FORMAT(created_at, "%Y-%m-%d %H:%i:00") as created_at
            from
              metric_history
            where
              sql_str = "{}"
              and created_at > DATE_SUB(now(), INTERVAL 30 MINUTE)
          ) as t
        group by
          created_at
      ) t2 ON t1.date = t2.created_at
			order by date asc
        "#, x.sql_str);
        let query_result_vec = conn.query_all(Statement::from_sql_and_values(DatabaseBackend::MySql, sub_sql.as_str(), vec![]))
            .await
            .map_err(anyhow::Error::new)
            ?;
        for x in query_result_vec {
            let max_duration:i64 = x.try_get("","max_duration").unwrap();
            let min_duration:i64 = x.try_get("","min_duration").unwrap();
            let exec_count:Decimal = x.try_get("","exec_count").unwrap();
            let cache_hit_count:Decimal = x.try_get("","cache_hit_count").unwrap();
            let avg_duration:Decimal = x.try_get("","avg_duration").unwrap();
            let date:String = x.try_get("","date").unwrap();
            info!("date:{:?},avg_duration:{:?},max_duration:{:?},min_duration:{:?},exec_count:{:?},cache_hit_count:{:?}",date,avg_duration,max_duration,min_duration,exec_count,cache_hit_count)
        }
        //
        // let avg_durations: Vec<Decimal> = query_result_vec.iter()
        //     .map(|x| x.try_get("", "avg_duration"))
        //     .filter(|x| x.is_ok())
        //     .map(|x| x.unwrap())
        //     .collect();
        // let max_durations: Vec<i64> = query_result_vec.iter()
        //     .map(|x| x.try_get("", "max_duration"))
        //     .filter(|x| x.is_ok())
        //     .map(|x| x.unwrap())
        //     .collect();
        // let min_durations: Vec<i64> = query_result_vec.iter()
        //     .map(|x| x.try_get("", "min_duration"))
        //     .filter(|x| x.is_ok())
        //     .map(|x| x.unwrap())
        //     .collect();
        //
        // let exec_counts: Vec<Decimal> = query_result_vec.iter()
        //     .map(|x| x.try_get("", "exec_count"))
        //     .filter(|x| x.is_ok())
        //     .map(|x| x.unwrap())
        //     .collect();
        // let cache_hit_counts: Vec<Decimal> = query_result_vec.iter()
        //     .map(|x| x.try_get("", "cache_hit_count"))
        //     .filter(|x| x.is_ok())
        //     .map(|x| x.unwrap())
        //     .collect();
        // let dates: Vec<String> = query_result_vec.iter()
        //     .map(|x| x.try_get("", "date"))
        //     .filter(|x| x.is_ok())
        //     .map(|x| x.unwrap())
        //     .collect();
        // x.avg_durations = avg_durations;
        // x.min_durations = min_durations;
        // x.max_durations = max_durations;
        // x.exec_counts = exec_counts;
        // x.cache_hit_counts = cache_hit_counts;
        // x.dates = dates;
        // final_metric_result_vec.push(x);
    }
    let data_wrapper = DataWrapper::success(PageResponse {
        list: final_metric_result_vec,
        total: total as i64,
    });
    Ok(HttpResponse::Ok().json(data_wrapper))
}