use std::collections::HashMap;

use futures::{
    stream::{FuturesUnordered, SelectNextSome},
    Future, StreamExt,
};
use uuid::Uuid;

use crate::{
    handler::{Handler, HandlerBox, HandlerWrapper},
    response::Response,
    website::Website,
};

pub trait OutputProcessor<Out> {
    async fn process(&mut self, out: Out) -> ();
}

pub struct Client<Data: Clone, Out, OP: OutputProcessor<Out>> {
    data: Data,
    websites: HashMap<Uuid, Website<Data, Out>>,
    output_processor: OP,
}

impl<Data, Out, OP> Client<Data, Out, OP>
where
    Data: Clone + Send + Sync + 'static,
    Out: 'static,
    OP: OutputProcessor<Out>,
{
    pub fn new(data: Data, op: OP) -> Self {
        Self {
            data: data,
            websites: Default::default(),
            output_processor: op,
        }
    }

    pub fn handle_website<T: Into<Website<Data, Out>>>(&mut self, t: T) {
        self.websites.insert(Uuid::new_v4(), t.into());
    }
}
