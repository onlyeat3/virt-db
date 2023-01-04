use serde::{Serialize,Deserialize};

#[derive(Debug,Serialize,Deserialize)]
pub struct LoginReq{
    pub username:String,
    pub password:String,
}

#[derive(Debug,Serialize,Deserialize)]
pub struct Role{
    #[serde(rename(serialize="roleName",deserialize="roleName"))]
    pub role_name:String,
    pub value:String,
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
    pub token:String,
}