use std::sync::{Arc};

use futures::{
    future::join_all,
    stream::{StreamExt},
};
use reqwest::Url;
use tokio::{
    sync::{mpsc},
    task::JoinHandle,
};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    handler::{Handler, HandlerBox, HandlerWrapper},
    next_action::{NextAction, NextActionVector},
    response::Response,
};

type Handle<Data, Out> = Vec<Box<dyn HandlerWrapper<Data, Out> + Send + Sync>>;

pub struct WebsiteBuilder<Data, Out> {
    starting_urls: &'static [&'static str],
    parallel_limit: usize,
    handler: Handle<Data, Out>,
}

impl<Data, Out> WebsiteBuilder<Data, Out>
where
    Data: Send + Sync + 'static,
    Out: 'static,
{
    pub fn parallel_limit(mut self, limit: usize) -> Self {
        self.parallel_limit = limit;
        self
    }

    pub async fn handle<T, H>(mut self, handler: H) -> Self
    where
        T: 'static,
        H: Handler<T, Data, Out> + Send + Sync + 'static,
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
}

impl<Data, Out> Website<Data, Out>
where
    Data: Send + Sync + 'static + Clone,
    Out: Send + 'static,
{
    async fn _worker(
        url: Url,
        data: Data,
        handler: Arc<Handle<Data, Out>>,
    ) -> NextActionVector<Data, Out> {
        let Ok(resp) = reqwest::get(url).await else {
            return Vec::new();
        }; // TODO: Error Handling

        let Ok(resp) = Response::from_reqwest(resp).await else {
            return Vec::new();
        }; // TODO: Same as above
        let resp = Arc::new(resp);

        let handlers = handler.iter().map(|handler| {
            let data = data.clone();
            let resp = resp.clone();
            handler.handle(resp, data)
        });
        join_all(handlers).await.into_iter().flatten().collect()
    }

    async fn _fetcher(
        parallel_limit: usize,
        cx: mpsc::Sender<(Url, Data)>,
        rx: mpsc::Receiver<(Url, Data)>,
        handlers: Arc<Handle<Data, Out>>,
        output_sender: mpsc::Sender<Out>,
    ) {
        let rec_stream: ReceiverStream<_> = rx.into();
        rec_stream
            .map(|(url, data)| {
                let handlers = handlers.clone();
                Self::_worker(url, data, handlers)
            })
            .buffer_unordered(parallel_limit)
            .for_each(|actions| {
                let output_sender = output_sender.clone();
                let cx = cx.clone();
                async move {
                    for next_action in actions {
                        match next_action {
                            NextAction::PipeOutput(output) => {
                                output_sender.send(output).await;
                            }
                            NextAction::Visit(pair) => {
                                cx.send(pair).await;
                            }
                        }
                    }
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
        let handlers = self.handler.clone();
        let parallel = self.parallel_limit;
        self.join_handler = Some(tokio::spawn(async move {
            Self::_fetcher(parallel, cx, rx, handlers, output_sender).await
        }))
    }
}
