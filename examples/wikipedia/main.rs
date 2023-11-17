#![feature(async_fn_in_trait)]

use futures::StreamExt;
use log::{debug, info};
use reqwest::Url;
use std::fmt::Debug;
use syphon::client::Client;
use syphon::extractor::{Data, SearchSelectors, Selector, UrlExtractor};
use syphon::next_action::WebsiteOutput;
use syphon::website::Website;
use tl::parse_query_selector;

#[derive(Default, Debug)]
enum Context {
    #[default]
    Index,
}
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

#[derive(SearchSelectors)]
struct TitleExtractor {
    #[select(selector = "h1", text)]
    title: String,
    #[select(selector = "#p-lang-btn-checkbox", attr = "aria-label")]
    language_count: String,
}

async fn from_title(Selector(title): Selector<TitleExtractor>) -> Option<Output> {
    let language = title
        .language_count
        .split_ascii_whitespace()
        .filter_map(|x| x.parse().ok())
        .next()?;
    Some(Output {
        title: title.title,
        language,
    })
}
#[derive(Debug)]
struct AnchorExtractor {
    anchor: Vec<String>,
}

impl SearchSelectors for AnchorExtractor {
    fn search(dom: &VDom) -> Option<Self> {
        let parser = dom.parser();
        let k = dom
            .query_selector("a")?
            .filter_map(|node| node.get(parser))
            .filter_map(|node| node.as_tag())
            .map(|tag| tag.attributes())
            .filter_map(|attr| attr.get("href").flatten())
            .map(|href| href.as_utf8_str().to_string())
            .filter(|url| url.starts_with("/wiki/"))
            .collect();
        Some(Self { anchor: k })
    }
}

async fn visit_next_urls(
    Selector(anchors): Selector<AnchorExtractor>,
    Data(_): Data<()>,
    UrlExtractor(url): UrlExtractor,
) -> Vec<Url> {
    anchors
        .anchor
        .into_iter()
        .filter_map(|u| url.join(&u).ok())
        .collect()
}

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();

    debug!("{:?}", parse_query_selector("main#content a"));

    let wikipedia: Website<(), Output> = Website::new()
        .start_with(
            Url::parse("https://en.wikipedia.org/wiki/Special:Random")
                .expect("Unable to parse starting Url"),
        )
        .parallel_limit(64)
        .handle(from_title)
        .handle(visit_next_urls)
        .into();

    Client::new(wikipedia)
        .stream()
        .for_each(|x| async move { info!("{:?}", x) })
        .await;
}
