#![feature(async_fn_in_trait)]

use futures::StreamExt;
use log::info;
use reqwest::Url;
use std::fmt::Debug;
use syphon::client::Client;
use syphon::extractor::{self, SearchSelectors, Selector};
use syphon::next_action::WebsiteOutput;
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
}

async fn from_title(Selector(title): Selector<TitleExtractor>) -> Option<Output> {
    let language = title
        .language_count
        .split_ascii_whitespace()
        .filter_map(|x| x.parse().ok())
        .next()
        .unwrap_or(0);
    Some(Output {
        title: title.title,
        language,
    })
}
#[derive(SearchSelectors, Debug)]
struct AnchorExtractor {
    #[select(sel = "#bodyContent a", attr = "href")]
    anchor: Vec<String>,
}

async fn visit_next_urls(
    Selector(anchors): Selector<AnchorExtractor>,
    extractor::Url(url): extractor::Url,
) -> Vec<Url> {
    anchors
        .anchor
        .into_iter()
        .filter_map(|u| url.join(&u).ok())
        .collect()
}

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "wikipedia");
    env_logger::init();

    let wikipedia: Website<(), Output> = Website::new()
        .start_with(
            Url::parse("https://en.wikipedia.org/wiki/Special:Random")
                .expect("Unable to parse starting Url"),
        )
        .parallel_limit(256)
        .handle(from_title)
        .handle(visit_next_urls)
        .into();

    let mut stream = Client::handle(wikipedia).stream();

    while let Some(o) = stream.next().await {
        if o.language >= 10 {
            info!("Popular: {:?}", o)
        }
    }
}
