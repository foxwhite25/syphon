use async_trait::async_trait;

use crate::response::{FromResponse, Response};

pub struct Url(pub reqwest::Url);

#[async_trait]
impl<Ctx> FromResponse<Ctx> for Url {
    async fn from_response(resp: &Response, _: &Ctx) -> Option<Self> {
        Some(Self(resp.url.clone()))
    }
}
