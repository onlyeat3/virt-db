use std::collections::HashMap;

use nom::combinator::iterator;
use redis::{Commands, RedisResult, RedisWrite, ToRedisArgs};
use sqlparser::ast::{Query, Statement};
use sqlparser::dialect::{Dialect, MySqlDialect};
use sqlparser::keywords::Keyword::FROM;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Token, Tokenizer};

mod tests {}

#[test]
fn test_redis() {
    // connect to redis
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_connection().unwrap();
    let numbers = vec![1, 2, 3, 5];
    let _: () = con.set("my_key", numbers).unwrap();
    let v: RedisResult<Vec<i32>> = con.get("my_key");
    println!("v:{:?}", v);
}

#[test]
fn test_serde() {
    let mut map: HashMap<String, &[u8]> = HashMap::new();
    let v = "第一个".as_bytes();
    map.insert(String::from("a"), v);
    let json_string = serde_json::to_string(&map).unwrap();
    println!("json_string:{}", json_string);

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_connection().unwrap();
    let _: () = con.set("my_key", json_string).unwrap();
    let v: RedisResult<String> = con.get("my_key");
    println!("v:{:?}", v);
}

#[test]
fn test_match() {
    let sql = "SELECT * from user_01 where ID = 123 and name = 'abc123'";
    let pattern = "select * from user where id = ? and name = ?";
    let dialect = MySqlDialect {}; // or AnsiDialect, or your own dialect ...

    let matched = is_pattern_match(sql, pattern, dialect);
    println!("pattern:{:?}\nsql:{:?}\neq:{:?}",pattern,sql,matched);
}