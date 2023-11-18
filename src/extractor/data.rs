use async_trait::async_trait;

use crate::response::{FromResponse, Response};

pub struct Context<Ctx>(pub Ctx);

#[async_trait]
impl<InnerCtx, OuterCtx> FromResponse<OuterCtx> for Context<InnerCtx>
where
    OuterCtx: Clone + Into<InnerCtx> + 'static + Sync,
{
    async fn from_response(_resp: &Response, data: &OuterCtx) -> Option<Self> {
        Some(Self(data.clone().into()))
    }
}
