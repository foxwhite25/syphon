use async_trait::async_trait;
use serde::de::DeserializeOwned;

use crate::response::{FromResponse, Response};

pub struct Json<T: DeserializeOwned>(pub T);

#[async_trait]
impl<T, Data> FromResponse<Data> for Json<T>
where
    T: DeserializeOwned,
{
    async fn from_response(resp: &Response, _data: &Data) -> Option<Self> {
        resp.json().map(|j| Self(j)).ok()
    }
}
