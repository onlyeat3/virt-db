use redis::cluster::{ClusterClient, ClusterConnection};
use redis::{Client, Commands, Connection, RedisResult};

pub enum SysRedisClient {
    Single(Connection),
    Cluster(ClusterConnection),
}

impl SysRedisClient {
    pub fn new(url: &str) -> RedisResult<Self> {
        let addrs: Vec<&str> = url.split(',').collect();
        if addrs.len() == 1 {
            let client = Client::open(url)?;
            let con = client.get_connection()?;
            Ok(Self::Single(con))
        } else {
            let client = ClusterClient::open(addrs)?;
            let con = client.get_connection()?;
            Ok(Self::Cluster(con))
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> RedisResult<()> {
        match self {
            Self::Single(con) => con.set(key, value),
            Self::Cluster(con) => con.set(key, value),
        }?;
        Ok(())
    }

    pub fn set_ex(&mut self, key: &str, value: Vec<u8>,seconds:usize) -> RedisResult<()> {
        match self {
            Self::Single(con) => con.set_ex(key, value.as_slice(),seconds),
            Self::Cluster(con) => con.set_ex(key, value.as_slice(),seconds),
        }?;
        Ok(())
    }

    pub fn get(&mut self, key: &str) -> RedisResult<String> {
        match self {
            Self::Single(con) => con.get(key),
            Self::Cluster(con) => con.get(key),
        }
    }

    pub fn mget(&mut self, keys: &[&str]) -> RedisResult<Vec<Option<String>>> {
        match self {
            Self::Single(con) => con.get(keys),
            Self::Cluster(con) => con.get(keys),
        }
    }

    pub fn exists(&mut self, key: &str) -> RedisResult<bool> {
        match self {
            Self::Single(con) => con.exists(key),
            Self::Cluster(con) => con.exists(key),
        }
    }
}