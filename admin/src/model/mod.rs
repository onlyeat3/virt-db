#[warn()]
pub mod user_model;

use serde::{Serialize, Deserialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataWrapper<V> {
    pub code: i32,
    pub message: String,
    pub success: bool,
    pub result: V,
}

impl<V> DataWrapper<V> {
    pub fn success(v: V) -> DataWrapper<V> {
        DataWrapper::result(0, String::from("OK"), v)
    }

    pub fn result(code: i32, message: String, v: V) -> DataWrapper<V> {
        DataWrapper {
            code,
            message,
            success: code == 0,
            result: v,
        }
    }
}