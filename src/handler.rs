use std::{marker::PhantomData, thread};

use async_trait::async_trait;
use futures::{future::BoxFuture, Future};

use crate::response::{FromResponse, Response};

pub(crate) trait HandlerWrapper<Data> {
    type Output;

    fn handle<'a, 'b>(
        &'a self,
        resp: &'b Response,
        data: Data,
    ) -> BoxFuture<'b, Option<Self::Output>>;
}

impl<H, T, Data> HandlerWrapper<Data> for HandlerBox<H, T, Data>
where
    Data: Send + Sync,
    H: Handler<T, Data> + Send + Sync + 'static,
    Data: 'static,
{
    type Output = H::Output;

    fn handle<'a, 'b>(
        &'a self,
        resp: &'b Response,
        data: Data,
    ) -> BoxFuture<'b, Option<Self::Output>> {
        let fut = self.inner.clone();
        let fut = async move { fut.handle(resp, data).await };
        Box::pin(fut)
    }
}

pub(crate) struct HandlerBox<H, T, Data>
where
    H: Handler<T, Data> + Send,
{
    inner: H,
    _marker: PhantomData<fn() -> (T, Data)>,
}

impl<H, T, Data> HandlerBox<H, T, Data>
where
    H: Handler<T, Data> + Send,
{
    pub(crate) fn from_handler(h: H) -> Self {
        Self {
            inner: h,
            _marker: Default::default(),
        }
    }
}

#[async_trait]
pub trait Handler<T, Data>: Clone + Send + Sized {
    type Output;

    async fn handle(self, resp: &Response, data: Data) -> Option<Self::Output>;
}

#[async_trait]
impl<F, Fut, Data, Out> Handler<(), Data> for F
where
    F: FnOnce() -> Fut + Send + Sync + 'static + Clone,
    Fut: Future<Output = Out> + Send,
    Data: Send + Sync + 'static,
{
    type Output = Out;

    async fn handle(self, _resp: &Response, _data: Data) -> Option<Self::Output> {
        Some(self().await.into())
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
        impl<F, Fut, Data, Out, $($ty,)*> Handler<($($ty,)*), Data> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Send + Sync + 'static + Clone,
            Fut: Future<Output = Out> + Send,
            Data: Send + Sync + 'static,
            $( $ty: FromResponse<Data> + Send, )*
        {
            type Output = Out;

            async fn handle(self, resp: &Response, data: Data) -> Option<Out> {
                let data = data;
                $(
                    let $ty = $ty::from_response(resp, &data).await?;
                )*
                Some(self($($ty,)*).await.into())
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
            Response {
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
            Response {
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
