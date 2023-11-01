use reqwest::Url;

pub(crate) type NextActionVector<Data, Output> = Vec<NextAction<Data, Output>>;

pub trait WebsiteOutput {
    fn should_process(&self) -> bool;
}

#[derive(PartialEq, Eq, Debug)]
pub struct NextUrl<Data> {
    pub(crate) url: Url,
    pub(crate) data: Data,
}

impl<Data> NextUrl<Data> {
    fn new(url: Url, data: Data) -> Self {
        Self { url, data }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum NextAction<Data, Out> {
    PipeOutput(Out),
    Visit(NextUrl<Data>),
}

impl<Data, Out> IntoNextActionVec<Data, Out> for NextUrl<Data>
where
    Out: WebsiteOutput,
{
    fn into_next_action_vec(self) -> NextActionVector<Data, Out> {
        vec![NextAction::Visit(self)]
    }
}

impl<Data, Out> IntoNextActionVec<Data, Out> for Url
where
    Data: Default,
    Out: WebsiteOutput,
{
    fn into_next_action_vec(self) -> NextActionVector<Data, Out> {
        vec![NextAction::Visit(NextUrl::new(self, Default::default()))]
    }
}

impl<Data, Out> IntoNextActionVec<Data, Out> for Out
where
    Out: WebsiteOutput,
{
    fn into_next_action_vec(self) -> NextActionVector<Data, Out> {
        vec![NextAction::PipeOutput(self)]
    }
}

impl<Data, Out> IntoNextActionVec<Data, Out> for Option<Out>
where
    Out: WebsiteOutput,
{
    fn into_next_action_vec(self) -> NextActionVector<Data, Out> {
        match self {
            Some(out) => {
                vec![NextAction::PipeOutput(out)]
            }
            None => {
                vec![]
            }
        }
    }
}

pub trait IntoNextActionVec<Data, Out>
where
    Out: WebsiteOutput,
{
    fn into_next_action_vec(self) -> NextActionVector<Data, Out>;
}

impl<Data, Out, T> IntoNextActionVec<Data, Out> for Vec<T>
where
    T: IntoNextActionVec<Data, Out>,
    Out: WebsiteOutput,
{
    fn into_next_action_vec(self) -> NextActionVector<Data, Out> {
        self.into_iter()
            .flat_map(IntoNextActionVec::into_next_action_vec)
            .collect()
    }
}

macro_rules! impl_into_response {
    (
        [$($ty:ident),*]
    ) => {
        #[allow(non_snake_case)]
        impl<Data, Out, $($ty,)*> IntoNextActionVec<Data, Out> for ($($ty),*,)
        where
            Out: WebsiteOutput,
            $( $ty: IntoNextActionVec<Data, Out>, )*
        {
            fn into_next_action_vec(self) -> NextActionVector<Data, Out> {
                let ($($ty),*,) = self;

                let parts = vec![$(
                    $ty.into_next_action_vec(),
                )*];

                parts.into_iter().flatten().collect()
            }
        }

    }
}
#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!([T1]);
        $name!([T1, T2]);
        $name!([T1, T2, T3]);
        $name!([T1, T2, T3, T4]);
        $name!([T1, T2, T3, T4, T5]);
        $name!([T1, T2, T3, T4, T5, T6]);
        $name!([T1, T2, T3, T4, T5, T6, T7]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16]);
    };
}

all_the_tuples!(impl_into_response);
