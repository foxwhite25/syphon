use std::{marker::PhantomData, pin::Pin, sync::Arc};

use async_trait::async_trait;
use futures::{future::BoxFuture, Future};


use crate::{
    next_action::{IntoNextActionVec, NextActionVector, WebsiteOutput},
    response::{FromResponse, Response},
};

pub(crate) trait HandlerWrapper<Data, Out> {
    fn handle<'a>(
        &'a self,
        resp: Arc<Response>,
        data: Data,
    ) -> BoxFuture<'static, NextActionVector<Data, Out>>;
}

impl<H, T, Data, Out> HandlerWrapper<Data, Out> for HandlerBox<H, T, Data, Out>
where
    Data: Send + Sync,
    H: Handler<T, Data, Out> + Send + Sync + 'static,
    Data: 'static,
{
    fn handle<'a>(
        &'a self,
        resp: Arc<Response>,
        data: Data,
    ) -> BoxFuture<'static, NextActionVector<Data, Out>> {
        let fut = self.inner.clone();
        let fut = async move { fut.handle(resp, data).await };
        Box::pin(fut)
    }
}

pub(crate) struct HandlerBox<H, T, Data, Out>
where
    H: Handler<T, Data, Out> + Send,
{
    inner: H,
    _marker: PhantomData<fn() -> (T, Data, Out)>,
}

impl<H, T, Data, Out> HandlerBox<H, T, Data, Out>
where
    H: Handler<T, Data, Out> + Send,
{
    pub(crate) fn from_handler(h: H) -> Self {
        Self {
            inner: h,
            _marker: Default::default(),
        }
    }
}

pub trait Handler<T, Data, Out>: Send + Clone {
    type Future: Future<Output = NextActionVector<Data, Out>> + Send + 'static;
    fn handle(self, resp: Arc<Response>, data: Data) -> Self::Future;
}

impl<F, Fut, FutOut, Data, Out> Handler<(), Data, Out> for F
where
    F: FnOnce() -> Fut + Send + Clone + 'static,
    Fut: Future<Output = FutOut> + Send,
    FutOut: IntoNextActionVec<Data, Out>,
    Out: WebsiteOutput + 'static,
    Data: 'static,
{
    type Future = Pin<Box<dyn Future<Output = NextActionVector<Data, Out>> + Send>>;

    fn handle(self, _resp: Arc<Response>, _data: Data) -> Self::Future {
        Box::pin(async move { self().await.into_next_action_vec() })
    }
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

macro_rules! impl_handler {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case, unused_mut)]
        #[async_trait]
        impl<F, Fut, FutOut, Data, Out, $($ty,)*> Handler<($($ty,)*), Data, Out> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = FutOut> + Send,
            FutOut: IntoNextActionVec<Data, Out>,
            Out: WebsiteOutput + 'static,
            Data: 'static + Send + Sync,
            $( $ty: FromResponse<Data> + Send, )*
        {
            type Future = Pin<Box<dyn Future<Output = NextActionVector<Data, Out>> + Send>>;

            fn handle(self, resp: Arc<Response>, data: Data) -> Self::Future {
                Box::pin(async move {
                    let data = data;
                    $(
                        let $ty = match $ty::from_response(resp.as_ref(), &data).await {
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

all_the_tuples!(impl_handler);

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::{
        extractor::{Data, Json, SearchSelectors, Selector},
        handler::Handler,
        response::Response,
    };
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Target {
        name: String,
    }

    struct D {
        bar: String,
    }

    struct Out {
        foo_bar: String,
    }

    #[tokio::test]
    async fn test_handler() {
        async fn foo(Json(target): Json<Target>, Data(data): Data<Arc<D>>) -> Out {
            Out {
                foo_bar: target.name + data.bar.as_str(),
            }
        }

        let out: Out = Handler::handle(
            foo,
            &Response {
                bytes: br#"{"name": "foo"}"#.to_vec(),
            },
            Arc::new(D {
                bar: "bar".to_string(),
            }),
        )
        .await
        .unwrap();

        assert_eq!(out.foo_bar, "foobar");

        #[derive(SearchSelectors)]
        struct Target2 {
            #[select(selector = "#foo", text)]
            _name: String,
        }

        async fn should_not_run(
            Selector(_selector): Selector<Target2>,
            Data(_data): Data<Arc<D>>,
        ) -> Out {
            unreachable!()
        }

        let out: Option<Out> = Handler::handle(
            should_not_run,
            &Response {
                bytes: br#"{"name": "foo"}"#.to_vec(),
            },
            Arc::new(D {
                bar: "bar".to_string(),
            }),
        )
        .await;
        assert!(out.is_none())
    }
}
