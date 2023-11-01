use std::sync::Arc;

use futures::StreamExt;
use log::debug;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::website::WebsiteWrapper;

pub trait OutputProcessor<Out> {
    async fn process(&mut self, out: Out);
}

pub struct Client<Out, OP: OutputProcessor<Out>> {
    websites: Vec<Box<dyn WebsiteWrapper<Out>>>,
    output_processor: OP,
}

impl<Out, OP> Client<Out, OP>
where
    Out: 'static,
    OP: OutputProcessor<Out>,
{
    pub fn new(op: OP) -> Self {
        Self {
            websites: Default::default(),
            output_processor: op,
        }
    }

    pub fn handle_website<T: WebsiteWrapper<Out> + 'static>(mut self, t: T) -> Self {
        self.websites.push(Box::new(t));
        self
    }

    pub async fn serve(mut self) {
        let (cx, mut rx) = mpsc::channel(16);
        for ele in &mut self.websites {
            let cx = cx.clone();
            ele.init(cx);
            ele.launch()
        }
        while let Some(output) = rx.recv().await {
            debug!("rec output");
            self.output_processor.process(output).await
        }
    }
}
