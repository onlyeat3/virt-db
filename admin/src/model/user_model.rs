use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResp {
    pub access_token: String,
    pub refresh_token: String,
    pub expires: String,
    pub roles: Vec<String>,
    pub username: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUserInfoResp {
    pub user_id: String,
    pub username: String,
    pub real_name: String,
    pub avatar: String,
    pub desc: String,
    pub password: String,
    pub token: String,
    pub home_path: String,
    pub roles: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialOrd, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordUpdateParam {
    pub current_password: String,
    pub new_password: String,
    pub new_password_confirm: String,
}