use async_trait::async_trait;
use serde::de::DeserializeOwned;

use crate::response::{FromResponse, Response};

pub struct Json<T: DeserializeOwned>(pub T);

#[async_trait]
impl<T, Ctx> FromResponse<Ctx> for Json<T>
where
    T: DeserializeOwned,
{
    async fn from_response(resp: &Response, _: &Ctx) -> Option<Self> {
        resp.json().map(|j| Self(j)).ok()
    }
}
