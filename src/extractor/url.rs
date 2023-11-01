use async_trait::async_trait;
use reqwest::Url;

use crate::response::{FromResponse, Response};

pub struct UrlExtractor(pub Url);

#[async_trait]
impl<Data> FromResponse<Data> for UrlExtractor {
    async fn from_response(resp: &Response, _data: &Data) -> Option<Self> {
        Some(Self(resp.url.clone()))
    }
}
