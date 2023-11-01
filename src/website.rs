use std::{fmt::Debug, sync::Arc};

use futures::future::join_all;
use hashbrown::HashSet;
use log::{debug, error};
use reqwest::{Method, Request, Url};
use tokio::{
    sync::{mpsc, Mutex, Semaphore},
    task::JoinHandle,
};

use crate::{
    handler::{Handler, HandlerBox, HandlerWrapper},
    next_action::{NextAction, NextActionVector, NextUrl},
    response::Response,
};

type Handle<Data, Out> = Vec<Box<dyn HandlerWrapper<Data, Out> + Send + Sync>>;

pub struct WebsiteBuilder<Data, Out> {
    starting_urls: Vec<Url>,
    parallel_limit: usize,
    handler: Handle<Data, Out>,
}

impl<Data, Out> WebsiteBuilder<Data, Out>
where
    Data: Send + 'static,
    Out: 'static,
{
    pub fn parallel_limit(mut self, limit: usize) -> Self {
        self.parallel_limit = limit;
        self
    }

    pub fn start_with(mut self, url: Url) -> Self {
        self.starting_urls.push(url);
        self
    }

    pub fn handle<T, H>(mut self, handler: H) -> Self
    where
        T: 'static,
        H: Handler<T, Data, Out> + Send + Sync + 'static,
    {
        let wrapper = HandlerBox::from_handler(handler);
        self.handler.push(Box::new(wrapper));
        self
    }
}

impl<Data, Out> From<WebsiteBuilder<Data, Out>> for Website<Data, Out> {
    fn from(val: WebsiteBuilder<Data, Out>) -> Self {
        Website {
            starting_urls: Arc::new(val.starting_urls),
            parallel_limit: val.parallel_limit,
            handler: Arc::from(val.handler),
            join_handler: None,
            sender: None,
            duplicate: Default::default(),
        }
    }
}

pub struct Website<Data, Out> {
    starting_urls: Arc<Vec<Url>>,
    parallel_limit: usize,
    handler: Arc<Handle<Data, Out>>,
    join_handler: Option<JoinHandle<()>>,
    sender: Option<mpsc::Sender<NextUrl<Data>>>,
    duplicate: Arc<Mutex<HashSet<String>>>,
}

impl<Data, Out> Website<Data, Out> {
    pub fn new() -> WebsiteBuilder<Data, Out> {
        WebsiteBuilder {
            starting_urls: Default::default(),
            parallel_limit: 16,
            handler: Default::default(),
        }
    }
}

pub trait WebsiteWrapper<Output> {
    fn init(&mut self, output_sender: mpsc::Sender<Output>);

    fn launch(&self);
}

impl<Data, Output> WebsiteWrapper<Output> for Website<Data, Output>
where
    Data: Clone + Send + 'static + Default + Sync + Debug,
    Output: Send + 'static + Debug,
{
    fn init(&mut self, output_sender: mpsc::Sender<Output>) {
        let (cx, rx) = mpsc::channel(self.parallel_limit * 4);
        let handlers = self.handler.clone();
        let parallel = self.parallel_limit;
        let duplicate = self.duplicate.clone();
        self.sender = Some(cx.clone());
        self.join_handler = Some(tokio::spawn(async move {
            duplicate.lock().await.clear();
            _fetcher(parallel, cx, rx, handlers, output_sender, duplicate).await
        }))
    }

    fn launch(&self) {
        let Some(sender) = self.sender.clone() else {
            return;
        };
        let starting_urls = self.starting_urls.clone();
        tokio::spawn(async move {
            for ele in starting_urls.iter() {
                let _ = sender
                    .send(NextUrl {
                        url: ele.clone(),
                        data: Default::default(),
                    })
                    .await;
            }
        });
    }
}

async fn _worker<Data, Out>(
    url: Url,
    data: Data,
    handler: Arc<Handle<Data, Out>>,
    client: reqwest::Client,
) -> NextActionVector<Data, Out>
where
    Data: Clone,
{
    let Ok(resp) = client.execute(Request::new(Method::GET, url)).await else {
            return Vec::new();
        }; // TODO: Error Handling/Expo. Backoff

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

async fn _fetcher<Data, Out>(
    parallel_limit: usize,
    cx: mpsc::Sender<NextUrl<Data>>,
    mut rx: mpsc::Receiver<NextUrl<Data>>,
    handlers: Arc<Handle<Data, Out>>,
    output_sender: mpsc::Sender<Out>,
    duplicate: Arc<Mutex<HashSet<String>>>,
) where
    Data: Clone + Debug + Send + Sync + 'static,
    Out: Debug + Send + 'static,
{
    let sem = Arc::new(Semaphore::new(parallel_limit));
    let client = reqwest::Client::builder().build().unwrap();
    while let Some(next) = rx.recv().await {
        let handlers = handlers.clone();
        let output_sender = output_sender.clone();
        let cx = cx.clone();
        let sem = sem.clone();
        let duplicate = duplicate.clone();
        let client = client.clone();
        tokio::spawn(async move {
            let Ok(permit) = sem.acquire_owned().await else {
                return;
            };
            let actions = _worker(next.url, next.data, handlers, client).await;
            for next_action in actions {
                debug!("next action: {:?}", next_action);
                match next_action {
                    NextAction::PipeOutput(output) => {
                        output_sender
                            .send(output)
                            .await
                            .unwrap_or_else(|err| error!("output_sender send error: {}", err));
                    }
                    NextAction::Visit(pair) => {
                        {
                            let mut lock = duplicate.lock().await;
                            if lock.get(pair.url.path()).is_some() {
                                continue;
                            }
                            lock.insert(pair.url.path().to_string());
                        }
                        cx.send(pair)
                            .await
                            .unwrap_or_else(|err| error!("next url send error: {}", err));
                    }
                }
            }
            drop(permit)
        });
    }
}
