use serde::{Serialize,Deserialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VtNodeRegisterParam{
    pub host:String,
    pub port:String,
}