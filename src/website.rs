use std::sync::{atomic::AtomicUsize, Arc};

use futures::{
    future::join_all,
    stream::{FuturesUnordered, StreamExt},
};
use reqwest::Url;
use tokio::{
    select,
    sync::{mpsc, RwLock},
    task::JoinHandle,
};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    handler::{Handler, HandlerBox, HandlerWrapper},
    response::Response,
};

type Handle<Data, Out> = Vec<Box<dyn HandlerWrapper<Data, Output = Out> + Send + Sync>>;

pub struct WebsiteBuilder<Data, Out> {
    starting_urls: &'static [&'static str],
    parallel_limit: usize,
    handler: Handle<Data, Out>,
}

impl<Data, Out> WebsiteBuilder<Data, Out>
where
    Data: Send + Sync + 'static,
{
    pub fn parallel_limit(mut self, limit: usize) -> Self {
        self.parallel_limit = limit;
        self
    }

    pub async fn handle<T, H>(mut self, handler: H) -> Self
    where
        T: 'static,
        H: Handler<T, Data, Output = Out> + Send + Sync + 'static,
    {
        let wrapper = HandlerBox::from_handler(handler);
        self.handler.push(Box::new(wrapper));
        self
    }
}

pub struct Website<Data, Out> {
    starting_urls: &'static [&'static str],
    parallel_limit: usize,
    handler: Arc<Handle<Data, Out>>,
    join_handler: Option<JoinHandle<()>>,
    sender: Option<mpsc::Sender<(Url, Data)>>,
}

impl<Data, Out> Website<Data, Out>
where
    Data: Send + Sync + 'static + Clone,
    Out: Send + 'static,
{
    async fn _worker(url: Url, data: Data, handler: Arc<Handle<Data, Out>>) -> Vec<Out> {
        let resp = Response {
            bytes: b"foo".to_vec(),
        };
        let handlers = handler.iter().map(|handler| {
            let data = data.clone();
            handler.handle(&resp, data)
        });
        join_all(handlers)
            .await
            .into_iter()
            .filter_map(|out| out)
            .collect()
    }

    async fn _fetcher(
        parallel_limit: usize,
        rx: mpsc::Receiver<(Url, Data)>,
        handlers: Arc<Handle<Data, Out>>,
        output_sender: mpsc::Sender<Out>,
    ) {
        let stream: ReceiverStream<_> = rx.into();
        stream
            .map(|(url, data)| {
                let handlers = handlers.clone();
                Self::_worker(url, data, handlers)
            })
            .buffer_unordered(parallel_limit)
            .for_each(|out| {
                let output_sender = output_sender.clone();
                async move {
                    let o = out.into_iter().map(|out| output_sender.send(out));
                    join_all(o).await;
                }
            })
            .await;
    }

    pub fn new(starting_urls: &'static [&'static str]) -> WebsiteBuilder<Data, Out> {
        WebsiteBuilder {
            starting_urls,
            parallel_limit: 16,
            handler: Default::default(),
        }
    }

    pub(crate) fn start(&mut self, output_sender: mpsc::Sender<Out>) {
        let (cx, rx) = mpsc::channel(self.parallel_limit);
        self.sender = Some(cx);
        let handlers = self.handler.clone();
        let parallel = self.parallel_limit;
        self.join_handler = Some(tokio::spawn(async move {
            Self::_fetcher(parallel, rx, handlers, output_sender).await
        }))
    }

    pub(crate) async fn visit(&self, url: Url, data: Data) {
        let Some(ref sender) = self.sender else {
            return;
        };
        sender.send((url, data)).await.unwrap()
    }
}
