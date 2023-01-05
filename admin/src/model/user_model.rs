use serde::{Serialize,Deserialize};

#[derive(Debug,Serialize,Deserialize)]
pub struct LoginReq{
    pub username:String,
    pub password:String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub role_name: String,
    pub value: String,
}

#[derive(Debug,Serialize,Deserialize)]
pub struct LoginResp{
    pub roles:Vec<Role>,
    #[serde(rename(serialize="userId",deserialize="userId"))]
    pub user_id:String,
    pub username:String,
    #[serde(rename(serialize="userId",deserialize="userId"))]
    pub real_name:String,
    pub desc:String,
    pub token:Option<String>,
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
    pub roles: Vec<Role>,
}