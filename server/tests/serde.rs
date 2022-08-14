use std::collections::HashMap;
use mysql_async::serde_json;
use redis::{Commands, RedisResult, RedisWrite, ToRedisArgs};
use sqlparser::keywords::Keyword::FROM;

mod tests{

}

#[test]
fn test_redis(){
    // connect to redis
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_connection().unwrap();
    let numbers = vec![1,2,3,5];
    let _ : () = con.set("my_key", numbers).unwrap();
    let v:RedisResult<Vec<i32>> = con.get("my_key");
    println!("v:{:?}",v);
}

#[test]
fn test_serde(){
    let mut map:HashMap<String,&[u8]>= HashMap::new();
    let v = "第一个".as_bytes();
    map.insert(String::from("a"),v);
    let json_string = serde_json::to_string(&map).unwrap();
    println!("json_string:{}", json_string);

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_connection().unwrap();
    let _ : () = con.set("my_key", json_string).unwrap();
    let v:RedisResult<String> = con.get("my_key");
    println!("v:{:?}",v);

}