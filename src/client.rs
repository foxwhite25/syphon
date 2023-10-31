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

    pub fn handle_website<T: WebsiteWrapper<Out> + 'static>(&mut self, t: T) {
        self.websites.push(Box::new(t));
    }
}
