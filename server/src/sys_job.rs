use std::collections::HashMap;
use std::time::Duration;

use chrono::Local;
use log::{debug, info};
use reqwest::{Body, Client, ClientBuilder, Error, Response};
use tokio::time;
use tokio::time::Instant;
use serde::{Serialize, Deserialize};

use sys_config::VirtDBConfig;

use crate::sys_config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataWrapper<V> {
    pub code: i32,
    pub message: String,
    pub success: bool,
    pub data: Option<V>,
}

pub async fn enable_node_live_refresh_job(sys_config: VirtDBConfig) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(1));
        interval.tick().await;
        info!("node_live_refresh_job started.");
        loop {
            interval.tick().await;
            let result = register(&sys_config).await;
            let _ = match result {
                Ok(response) => {
                    if response.status().as_u16() != 200 {
                        warn!("register vt_node fail. response:{:?}",response);
                        return;
                    }
                    let data_wrapper_result = response.json::<DataWrapper<String>>()
                        .await;
                    match data_wrapper_result {
                        Ok(data_wrapper) => {
                            if !data_wrapper.success{
                                warn!("register vt_node fail. response:{:?}",data_wrapper);
                            }
                        }
                        Err(e) => {
                            warn!("register vt_node fail:{:?}",e);
                        }
                    };
                }
                Err(e) => {
                    warn!("register vt_node fail:{:?}",e);
                }
            };
            time::sleep(Duration::from_secs(30)).await;
        }
    });
}

async fn register(sys_config: &VirtDBConfig) -> Result<Response, Error> {
    let address = sys_config.admin.address.clone();
    let register_api_url = format!("{}/vt_node/register", address);
    let mut params = HashMap::new();
    params.insert("port", sys_config.server.port.to_string());

    let request_body = serde_json::to_string(&params).unwrap();
    debug!("[job]request body:{}",request_body);
    let response = Client::builder()
        .no_proxy()
        .build()?
        .post(register_api_url)
        .header("Content-Type", "application/json")
        .body(request_body)
        .send()
        .await?;
    return Ok(response);
}

