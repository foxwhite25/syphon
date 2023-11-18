use std::{fmt::Debug, marker::PhantomData, sync::Arc};

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

type Handle<Ctx, Out> = Vec<Box<dyn HandlerWrapper<Ctx, Out> + Send + Sync>>;

pub struct WebsiteBuilder<Ctx, Out> {
    starting_urls: Vec<Url>,
    parallel_limit: usize,
    handler: Handle<Ctx, Out>,
}

impl<Ctx, Out> WebsiteBuilder<Ctx, Out>
where
    Ctx: Send + 'static,
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
        H: Handler<T, Ctx, Out> + Send + Sync + 'static,
    {
        let wrapper = HandlerBox::from_handler(handler);
        self.handler.push(Box::new(wrapper));
        self
    }
}

impl<Ctx, Out> From<WebsiteBuilder<Ctx, Out>> for Website<Ctx, Out> {
    fn from(val: WebsiteBuilder<Ctx, Out>) -> Self {
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

pub struct Website<Ctx, Out> {
    starting_urls: Arc<Vec<Url>>,
    parallel_limit: usize,
    handler: Arc<Handle<Ctx, Out>>,
    join_handler: Option<JoinHandle<()>>,
    sender: Option<mpsc::Sender<NextUrl<Ctx>>>,
    duplicate: Arc<Mutex<HashSet<String>>>,
}

impl<Ctx, Out> Website<Ctx, Out> {
    pub fn new() -> WebsiteBuilder<Ctx, Out> {
        WebsiteBuilder {
            starting_urls: Default::default(),
            parallel_limit: 16,
            handler: Default::default(),
        }
    }
}

pub struct WebsitePair<T1, T2, Out>(T1, T2, PhantomData<Out>)
where
    T1: WebsiteWrapper<Out>,
    T2: WebsiteWrapper<Out>;

pub trait WebsiteWrapper<Output> {
    fn init(&mut self, output_sender: mpsc::Sender<Output>);

    fn launch(&self);

    fn pair<T: WebsiteWrapper<Output>>(self, other: T) -> WebsitePair<T, Self, Output>
    where
        Self: Sized,
    {
        WebsitePair(other, self, Default::default())
    }
}

impl<Output, T1, T2> WebsiteWrapper<Output> for WebsitePair<T1, T2, Output>
where
    T1: WebsiteWrapper<Output>,
    T2: WebsiteWrapper<Output>,
{
    fn init(&mut self, output_sender: mpsc::Sender<Output>) {
        self.0.init(output_sender.clone());
        self.1.init(output_sender)
    }

    fn launch(&self) {
        self.0.launch();
        self.1.launch()
    }
}

impl<Ctx, Output> WebsiteWrapper<Output> for Website<Ctx, Output>
where
    Ctx: Clone + Send + 'static + Default + Sync + Debug,
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

async fn _worker<Ctx, Out>(
    url: Url,
    data: Ctx,
    handler: Arc<Handle<Ctx, Out>>,
    client: reqwest::Client,
) -> NextActionVector<Ctx, Out>
where
    Ctx: Clone,
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

async fn _fetcher<Ctx, Out>(
    parallel_limit: usize,
    cx: mpsc::Sender<NextUrl<Ctx>>,
    mut rx: mpsc::Receiver<NextUrl<Ctx>>,
    handlers: Arc<Handle<Ctx, Out>>,
    output_sender: mpsc::Sender<Out>,
    duplicate: Arc<Mutex<HashSet<String>>>,
) where
    Ctx: Clone + Debug + Send + Sync + 'static,
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
