use futures::StreamExt;
use log::{info, warn};
use reqwest::Url;
use std::fmt::Debug;
use syphon::client::Client;
use syphon::extractor::{self, SearchSelectors, Selector};
use syphon::next_action::{IntoNextActionVec, NextAction, WebsiteOutput};
use syphon::website::Website;

#[derive(Debug)]
struct Output {
    title: String,
    language: usize,
}

impl WebsiteOutput for Output {
    fn should_process(&self) -> bool {
        !self.title.is_empty()
    }
}

#[derive(SearchSelectors, Debug)]
struct TitleExtractor {
    #[select(sel = "h1", text)]
    title: String,
    #[select(sel = "#p-lang-btn-checkbox", attr = "aria-label")]
    language_count: String,
    #[select(sel = "#bodyContent a", attr = "href")]
    anchor: Vec<String>,
}

async fn from_title(
    Selector(title): Selector<TitleExtractor>,
    extractor::Url(url): extractor::Url,
) -> Vec<NextAction<(), Output>> {
    let mut next_urls = title
        .anchor
        .into_iter()
        .filter_map(|u| url.join(&u).ok())
        .collect::<Vec<_>>()
        .into_next_action_vec();

    let language = title
        .language_count
        .split_ascii_whitespace()
        .filter_map(|x| x.parse().ok())
        .next()
        .unwrap_or(0);

    next_urls.push(NextAction::PipeOutput(Output {
        title: title.title,
        language,
    }));
    next_urls
}

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "syphon,wikipedia");
    env_logger::init();

    let wikipedia: Website<(), Output, _> = Website::handle(from_title)
        .start_with(
            Url::parse("https://en.wikipedia.org/wiki/Special:Random")
                .expect("Unable to parse starting Url"),
        )
        .parallel_limit(256)
        .into();

    let mut stream = Client::handle(wikipedia).stream();

    while let Some(o) = stream.next().await {
        info!("{:?}", o);
    }
}
