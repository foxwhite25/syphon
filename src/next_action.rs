use reqwest::Url;

pub(crate) type NextActionVector<Data, Output> = Vec<NextAction<Data, Output>>;

pub trait WebsiteOutput {
    fn should_process(&self) -> bool;
}

pub enum NextAction<Data, Out> {
    PipeOutput(Out),
    Visit((Url, Data)),
}
pub trait IntoNextAction<Data, Out>
where
    Out: WebsiteOutput,
{
    fn into_next_action(self) -> NextAction<Data, Out>;
}

impl<Data, Out> IntoNextAction<Data, Out> for (Url, Data)
where
    Out: WebsiteOutput,
{
    fn into_next_action(self) -> NextAction<Data, Out> {
        NextAction::Visit(self)
    }
}

impl<Data, Out> IntoNextAction<Data, Out> for Url
where
    Data: Default,
    Out: WebsiteOutput,
{
    fn into_next_action(self) -> NextAction<Data, Out> {
        NextAction::Visit((self, Default::default()))
    }
}

impl<Data, Out> IntoNextAction<Data, Out> for Out
where
    Out: WebsiteOutput,
{
    fn into_next_action(self) -> NextAction<Data, Out> {
        NextAction::PipeOutput(self)
    }
}

pub trait IntoNextActionVec<Data, Out>
where
    Out: WebsiteOutput,
{
    fn into_next_action_vec(self) -> NextActionVector<Data, Out>;
}

auto trait NotVector {}

impl<T> !NotVector for Vec<T> {}

impl<Data, Out, T> IntoNextActionVec<Data, Out> for Vec<T>
where
    T: IntoNextAction<Data, Out>,
    Out: WebsiteOutput,
{
    fn into_next_action_vec(self) -> NextActionVector<Data, Out> {
        self.into_iter()
            .map(IntoNextAction::into_next_action)
            .collect()
    }
}

impl<Data, Out, T> IntoNextActionVec<Data, Out> for T
where
    T: IntoNextAction<Data, Out> + NotVector,
    Out: WebsiteOutput,
{
    fn into_next_action_vec(self) -> NextActionVector<Data, Out> {
        vec![self.into_next_action()]
    }
}
