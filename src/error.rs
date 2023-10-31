use std::result::Result as StdResult;
use thiserror::Error;

pub type Result<T> = StdResult<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("json serde encountered a unexpected token")]
    JsonError(#[from] serde_json::Error),
    #[error("reqwest error")]
    ReqwestError(#[from] reqwest::Error),
}
