use crate::response::{FromResponse, Response};
use async_trait::async_trait;

use scraper::Html;
pub use syphon_macro::SearchSelectors;
pub trait SearchSelectors: Sized {
    fn search(dom: &Html) -> Option<Self>;
}

pub struct Selector<T: SearchSelectors>(pub T);

#[async_trait]
impl<T, Data> FromResponse<Data> for Selector<T>
where
    T: SearchSelectors,
{
    async fn from_response(resp: &Response, _data: &Data) -> Option<Self> {
        let dom = Html::parse_document(std::str::from_utf8(&resp.bytes).ok()?);
        T::search(&dom).map(|x| Self(x))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(SearchSelectors, Debug)]
    struct Target {
        #[select(sel = "#text", text)]
        target: Vec<String>,
        #[select(sel = "#attr", attr = "href")]
        anchor: String,
        #[select(sel = "#nothing", text)]
        nothing: Option<String>,
    }

    #[test]
    fn test_search_selector_derive() {
        let source = r#"<p id="text">Hello</p><a id="attr", href="/post"></a>"#;
        let dom = Html::parse_fragment(source);
        let target = Target::search(&dom).unwrap();
        assert_eq!(target.target, vec!["Hello"]);
        assert_eq!(target.anchor, "/post");
        assert_eq!(target.nothing, None);
    }
}
