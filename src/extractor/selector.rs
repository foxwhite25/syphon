use crate::response::{FromResponse, Response};
use async_trait::async_trait;
use tl::{ParserOptions, VDom};

pub use syphon_macro::SearchSelectors;
pub trait SearchSelectors: Sized {
    fn search(dom: &VDom) -> Option<Self>;
}

pub struct Selector<T: SearchSelectors>(pub T);

#[async_trait]
impl<T, Data> FromResponse<Data> for Selector<T>
where
    T: SearchSelectors,
{
    async fn from_response(resp: &Response, _data: &Data) -> Option<Self> {
        let dom = tl::parse(
            std::str::from_utf8(&resp.bytes).ok()?,
            ParserOptions::new().track_ids().track_classes(),
        )
        .ok()?;
        T::search(&dom).map(|x| Self(x))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_search_selector_derive() {
        #[derive(SearchSelectors, Debug)]
        struct Target {
            #[select(selector = "#text", text)]
            target: String,
            #[select(selector = "#attr", attr = "href")]
            anchor: String,
        }
        let source = r#"<p id="text">Hello</p><a id="attr", href="/post"></a>"#;
        let dom = tl::parse(source, Default::default()).unwrap();
        let target = Target::search(&dom).unwrap();
        assert_eq!(target.target, "Hello");
        assert_eq!(target.anchor, "/post")
    }
}
