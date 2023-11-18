use std::{marker::PhantomData, pin::Pin, sync::Arc};

use async_trait::async_trait;
use futures::{future::BoxFuture, Future};

use crate::{
    next_action::{IntoNextActionVec, NextActionVector, WebsiteOutput},
    response::{FromResponse, Response},
};

pub(crate) trait HandlerWrapper<Ctx, Out> {
    fn handle(
        &self,
        resp: Arc<Response>,
        ctx: Ctx,
    ) -> BoxFuture<'static, NextActionVector<Ctx, Out>>;
}

impl<H, T, Ctx, Out> HandlerWrapper<Ctx, Out> for HandlerBox<H, T, Ctx, Out>
where
    Ctx: Send,
    H: Handler<T, Ctx, Out> + Send + 'static,
    Ctx: 'static,
{
    fn handle(
        &self,
        resp: Arc<Response>,
        ctx: Ctx,
    ) -> BoxFuture<'static, NextActionVector<Ctx, Out>> {
        let fut = self.inner.clone();
        let fut = async move { fut.handle(resp, ctx).await };
        Box::pin(fut)
    }
}

pub(crate) struct HandlerBox<H, T, Ctx, Out>
where
    H: Handler<T, Ctx, Out> + Send,
{
    inner: H,
    _marker: PhantomData<fn() -> (T, Ctx, Out)>,
}

impl<H, T, Ctx, Out> HandlerBox<H, T, Ctx, Out>
where
    H: Handler<T, Ctx, Out> + Send,
{
    pub(crate) fn from_handler(h: H) -> Self {
        Self {
            inner: h,
            _marker: Default::default(),
        }
    }
}

pub trait Handler<T, Ctx, Out>: Send + Clone {
    type Future: Future<Output = NextActionVector<Ctx, Out>> + Send + 'static;
    fn handle(self, resp: Arc<Response>, ctx: Ctx) -> Self::Future;
}

impl<F, Fut, FutOut, Ctx, Out> Handler<(), Ctx, Out> for F
where
    F: FnOnce() -> Fut + Send + Clone + 'static,
    Fut: Future<Output = FutOut> + Send,
    FutOut: IntoNextActionVec<Ctx, Out>,
    Out: WebsiteOutput + 'static,
    Ctx: 'static,
{
    type Future = Pin<Box<dyn Future<Output = NextActionVector<Ctx, Out>> + Send>>;

    fn handle(self, _resp: Arc<Response>, _ctx: Ctx) -> Self::Future {
        Box::pin(async move { self().await.into_next_action_vec() })
    }
}

macro_rules! impl_handler {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case, unused_mut)]
        #[async_trait]
        impl<F, Fut, FutOut, Ctx, Out, $($ty,)*> Handler<($($ty,)*), Ctx, Out> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = FutOut> + Send,
            FutOut: IntoNextActionVec<Ctx, Out>,
            Out: WebsiteOutput + 'static,
            Ctx: 'static + Send + Sync,
            $( $ty: FromResponse<Ctx> + Send, )*
        {
            type Future = Pin<Box<dyn Future<Output = NextActionVector<Ctx, Out>> + Send>>;

            fn handle(self, resp: Arc<Response>, ctx: Ctx) -> Self::Future {
                Box::pin(async move {
                    let ctx = ctx;
                    $(
                        let $ty = match $ty::from_response(resp.as_ref(), &ctx).await {
                            Some($ty) => $ty,
                            _ => { return Vec::new() }
                        };
                    )*
                    self($($ty,)*).await.into_next_action_vec()
                })
            }
        }
    };
}

#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!([T1]);
        $name!([T1, T2]);
        $name!([T1, T2, T3]);
        $name!([T1, T2, T3, T4]);
        $name!([T1, T2, T3, T4, T5]);
        $name!([T1, T2, T3, T4, T5, T6]);
        $name!([T1, T2, T3, T4, T5, T6, T7]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16]);
    };
}
all_the_tuples!(impl_handler);
