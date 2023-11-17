use std::marker::PhantomData;

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::website::{WebsitePair, WebsiteWrapper};

pub struct Client<Out, Websites>
where
    Websites: WebsiteWrapper<Out>,
{
    websites: Websites,
    _marker: PhantomData<Out>,
}

impl<Out, Website> Client<Out, Website>
where
    Out: 'static,
    Website: WebsiteWrapper<Out>,
{
    pub fn new(websites: Website) -> Self {
        Self {
            websites,
            _marker: Default::default(),
        }
    }

    pub fn handle_website<T: WebsiteWrapper<Out> + 'static>(
        self,
        t: T,
    ) -> Client<Out, WebsitePair<T, Website, Out>> {
        Client {
            websites: self.websites.pair(t),
            _marker: Default::default(),
        }
    }

    pub fn stream(mut self) -> ReceiverStream<Out> {
        let (cx, rx) = mpsc::channel(16);
        self.websites.init(cx);
        self.websites.launch();
        rx.into()
    }
}
