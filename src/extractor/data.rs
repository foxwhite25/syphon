use async_trait::async_trait;

use crate::response::{FromResponse, Response};

pub struct Data<D>(pub D);

#[async_trait]
impl<InnerData, OuterData> FromResponse<OuterData> for Data<InnerData>
where
    OuterData: Clone + Into<InnerData> + 'static + Sync,
{
    async fn from_response(_resp: &Response, data: &OuterData) -> Option<Self> {
        Some(Self(data.clone().into()))
    }
}
