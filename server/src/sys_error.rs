use std::io;
use std::io::Error;
use redis::RedisError;
use crate::protocol::ConnWriter;

#[derive(Debug)]
pub enum VirtDBMySQLError {
    ClientReadError(Error),
    ClientWriteError(Error),
    ServerReadError(Error),
    ServerWriteError(Error),

    RedisError(redis::RedisError),
    Other(anyhow::Error),
}

impl VirtDBMySQLError {
    pub async fn handle(
        self,
        client_writer:ConnWriter
    ) -> Result<(), VirtDBMySQLError> {
        match self {
            VirtDBMySQLError::ClientReadError(_) => {
                Ok(())
            }
            VirtDBMySQLError::ClientWriteError(_) => {
                Ok(())
            }
            VirtDBMySQLError::ServerReadError(_) => {
                Ok(())
            }
            VirtDBMySQLError::ServerWriteError(_) => {
                Ok(())
            }
            VirtDBMySQLError::RedisError(_) => {
                Ok(())
            }
            VirtDBMySQLError::Other(_) => {
                Ok(())
            }
        }
    }
}

impl From<redis::RedisError> for VirtDBMySQLError {
    fn from(e: RedisError) -> Self {
        VirtDBMySQLError::RedisError(e)
    }
}

impl VirtDBMySQLError {
    pub fn to_string(self) -> String {
        return match self {
            VirtDBMySQLError::RedisError(err) => err.to_string(),
            VirtDBMySQLError::Other(err) => err.to_string(),
            err => format!("{:?}",err),
        };
    }
}
