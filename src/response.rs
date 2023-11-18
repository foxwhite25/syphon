use async_trait::async_trait;
use serde::de::DeserializeOwned;

use reqwest::{Response as ReqwestResponse, Url};

use crate::error::{self, Result};

#[derive(Clone)]
pub struct Response {
    pub bytes: Vec<u8>,
    pub url: Url,
}

impl Response {
    pub(crate) async fn from_reqwest(value: ReqwestResponse) -> error::Result<Self> {
        Ok(Self {
            url: value.url().clone(),
            bytes: value.bytes().await?.to_vec(),
        })
    }

    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.bytes).map_err(|err| err.into())
    }
}
#[async_trait]
pub trait FromResponse<Ctx>: Sized {
    async fn from_response(resp: &Response, ctx: &Ctx) -> Option<Self>;
}
