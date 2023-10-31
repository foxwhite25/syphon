use async_trait::async_trait;
use serde::de::DeserializeOwned;

use crate::error::Result;

#[derive(Clone)]
pub struct Response {
    pub bytes: Vec<u8>,
}

impl Response {
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.bytes).map_err(|err| err.into())
    }
}
#[async_trait]
pub trait FromResponse<Data>: Sized {
    async fn from_response(resp: &Response, data: &Data) -> Option<Self>;
}
