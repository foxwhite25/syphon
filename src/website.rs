use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use futures::future::join_all;
use hashbrown::HashSet;
use log::{debug, error, info, warn};
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

pub struct WebsiteBuilder<Ctx, Out, Handler>
where
    Handler: HandlerWrapper<Ctx, Out>,
{
    starting_urls: Vec<Url>,
    parallel_limit: usize,
    handler: Handler,
    _maker: PhantomData<fn() -> (Ctx, Out)>,
}

impl<Ctx, Out, Handle> WebsiteBuilder<Ctx, Out, Handle>
where
    Ctx: Send + 'static + Clone + Sync,
    Out: 'static + Send,
    Handle: HandlerWrapper<Ctx, Out> + Send + Sync,
{
    pub fn parallel_limit(mut self, limit: usize) -> Self {
        self.parallel_limit = limit;
        self
    }

    pub fn start_with(mut self, url: Url) -> Self {
        self.starting_urls.push(url);
        self
    }

    pub fn and<T, H>(self, handler: H) -> WebsiteBuilder<Ctx, Out, impl HandlerWrapper<Ctx, Out>>
    where
        T: 'static,
        H: crate::handler::Handler<T, Ctx, Out> + Send + Sync + 'static,
    {
        let wrapper = HandlerBox::from_handler(handler);
        WebsiteBuilder {
            starting_urls: self.starting_urls,
            parallel_limit: self.parallel_limit,
            handler: self.handler.pair(wrapper),
            _maker: Default::default(),
        }
    }
}

impl<Ctx, Out, Handler> From<WebsiteBuilder<Ctx, Out, Handler>> for Website<Ctx, Out, Handler>
where
    Handler: HandlerWrapper<Ctx, Out>,
{
    fn from(val: WebsiteBuilder<Ctx, Out, Handler>) -> Self {
        Website {
            starting_urls: Arc::new(val.starting_urls),
            parallel_limit: val.parallel_limit,
            handler: Arc::from(val.handler),
            join_handler: None,
            sender: None,
            _maker: Default::default(),
        }
    }
}

pub struct Website<Ctx, Out, Handler>
where
    Handler: HandlerWrapper<Ctx, Out>,
{
    starting_urls: Arc<Vec<Url>>,
    parallel_limit: usize,
    handler: Arc<Handler>,
    join_handler: Option<JoinHandle<()>>,
    sender: Option<mpsc::Sender<NextUrl<Ctx>>>,
    _maker: PhantomData<fn() -> Out>,
}

impl<Ctx, Out, T, Handler> Website<Ctx, Out, HandlerBox<Handler, T, Ctx, Out>>
where
    Handler: crate::handler::Handler<T, Ctx, Out>,
    Ctx: std::marker::Send,
{
    pub fn handle(handler: Handler) -> WebsiteBuilder<Ctx, Out, HandlerBox<Handler, T, Ctx, Out>> {
        WebsiteBuilder {
            starting_urls: Default::default(),
            parallel_limit: 16,
            handler: HandlerBox::from_handler(handler),
            _maker: Default::default(),
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

impl<Ctx, Output, Handler> WebsiteWrapper<Output> for Website<Ctx, Output, Handler>
where
    Ctx: Clone + Send + 'static + Default + Sync + Debug,
    Output: Send + 'static + Debug,
    Handler: HandlerWrapper<Ctx, Output> + Send + Sync + 'static,
{
    fn init(&mut self, output_sender: mpsc::Sender<Output>) {
        let (cx, rx) = mpsc::channel(self.parallel_limit * 4);
        let handlers = self.handler.clone();
        let parallel = self.parallel_limit;
        self.sender = Some(cx.clone());
        self.join_handler = Some(tokio::spawn(async move {
            _fetcher(
                parallel,
                cx,
                rx,
                handlers,
                output_sender,
                Default::default(),
            )
            .await
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

async fn _worker<Ctx, Out, Handler>(
    url: Url,
    data: Ctx,
    handler: Arc<Handler>,
    client: reqwest::Client,
) -> NextActionVector<Ctx, Out>
where
    Ctx: Clone,
    Handler: HandlerWrapper<Ctx, Out>,
{
    let Ok(resp) = client.execute(Request::new(Method::GET, url.clone())).await else {
        return Vec::new();
    }; // TODO: Error Handling/Expo. Backoff

    if resp.status() != 200 {
        warn!("{} responsed with {}", url, resp.status());
    }

    let Ok(resp) = Response::from_reqwest(resp).await else {
        return Vec::new();
    }; // TODO: Same as above

    let resp = Arc::new(resp);

    handler.handle(resp.clone(), data.clone()).await
}

async fn _fetcher<Ctx, Out, Handler>(
    parallel_limit: usize,
    cx: mpsc::Sender<NextUrl<Ctx>>,
    mut rx: mpsc::Receiver<NextUrl<Ctx>>,
    handlers: Arc<Handler>,
    output_sender: mpsc::Sender<Out>,
    duplicate: Arc<scc::HashSet<String>>,
) where
    Handler: HandlerWrapper<Ctx, Out> + Send + Sync + 'static,
    Ctx: Clone + Debug + Send + Sync + 'static,
    Out: Debug + Send + 'static,
{
    let sem = Arc::new(Semaphore::new(parallel_limit));
    let client = reqwest::Client::builder().build().unwrap();
    while let Some(next) = rx.recv().await {
        let handlers = handlers.clone();
        let output_sender = output_sender.clone();
        let cx = cx.clone();
        let permit = sem.clone().acquire_owned().await.unwrap();
        let duplicate = duplicate.clone();
        let client = client.clone();
        tokio::spawn(async move {
            let url = Arc::new(next.url.host().map(|x| x.to_string()));
            let actions = _worker(next.url, next.data, handlers, client).await;
            drop(permit);
            let futs = actions.into_iter().map(move |next_action| {
                let output_sender = output_sender.clone();
                let duplicate = duplicate.clone();
                let cx = cx.clone();
                let url = url.clone();
                async move {
                    match next_action {
                        NextAction::PipeOutput(output) => {
                            output_sender
                                .send(output)
                                .await
                                .unwrap_or_else(|err| error!("output_sender send error: {}", err));
                        }
                        NextAction::Visit(pair) => {
                            if !pair.url.host().map(|x| x.to_string()).eq(&url) {
                                return;
                            }
                            if duplicate
                                .insert_async(pair.url.path().to_string())
                                .await
                                .is_err()
                            {
                                return;
                            };
                            cx.send(pair)
                                .await
                                .unwrap_or_else(|err| error!("next url send error: {}", err));
                        }
                        NextAction::None => {}
                    }
                }
            });
            tokio::spawn(async move { join_all(futs).await });
        });
    }
}
