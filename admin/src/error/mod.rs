use std::fmt::{Debug, Display, Formatter};

pub struct SysError {
    err: anyhow::Error,
}

impl Debug for SysError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Error")
            .field("err", &self.err)
            .finish()
    }
}

impl Display for SysError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Error")
            .field("err", &self.err)
            .finish()
    }
}

impl actix_web::error::ResponseError for SysError {

}

impl From<anyhow::Error> for SysError {
    fn from(err: anyhow::Error) -> SysError {
        SysError { err }
    }
}