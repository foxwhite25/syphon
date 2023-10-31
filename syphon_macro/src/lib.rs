use std::collections::HashMap;

use itertools::Itertools;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Meta};

#[proc_macro_derive(SearchSelectors, attributes(select))]
pub fn derive_search_selector(item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as DeriveInput);
    let struct_name = &ast.ident;
    let fields = if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(ref fields),
        ..
    }) = ast.data
    {
        fields
    } else {
        panic!("Only support Struct")
    };

    enum OutputType {
        Text,
        Attr(String),
    }

    impl Into<String> for OutputType {
        fn into(self) -> String {
            match self {
                OutputType::Text => r#".inner_text(parser)"#.to_string(),
                OutputType::Attr(name) => {
                    format!(r#".as_tag()?.attributes().get("{}")??.as_utf8_str()"#, name)
                }
            }
        }
    }

    let (name, sel, output): (Vec<_>, Vec<_>, Vec<_>) = fields
        .named
        .iter()
        .map(|x| {
            let name = x.ident.as_ref().unwrap();
            let select = x
                .attrs
                .iter()
                .filter_map(|attr| match &attr.meta {
                    Meta::List(l) => Some(l),
                    _ => None,
                })
                .find(|attr| {
                    let seg = &attr.path.segments;
                    let Some(first) = seg
                .first() else {
                    return false;
                };
                    first.ident.to_string() == "select"
                })
                .unwrap_or_else(|| panic!("Unable to find attr \"select\" on field \"{name}\" "));

            let select = select.tokens.to_string();
            let attrs = select
                .split(",")
                .map(|s| s.trim())
                .map(|trimmed| match trimmed.split_once("=") {
                    Some(x) => x,
                    None => (trimmed, "true"),
                })
                .map(|(a, b)| (a.trim(), b.trim()))
                .collect::<HashMap<_, _>>();

            let sel = attrs
                .get("selector")
                .map(|s| s.trim_matches('"'))
                .unwrap_or_else(|| {
                    panic!("Unable to find required field \"selector\" on \"{name}\"")
                })
                .to_string();
            let text = attrs
                .get("text")
                .map(|s| s.parse().ok())
                .flatten()
                .unwrap_or(false);
            let attr = attrs.get("attr").map(|s| *s).map(|s| s.trim_matches('"'));

            if text && attr.is_some() {
                panic!("\"attr\" and \"text\" are mutually exclusive")
            }
            if !text && attr.is_none() {
                panic!("either \"text\" or \"attr\" need to be set")
            }
            let output: String = if let Some(attr) = attr {
                OutputType::Attr(attr.to_string())
            } else {
                OutputType::Text
            }
            .into();
            let output: proc_macro2::TokenStream = output.parse().unwrap();
            (name, sel, output)
        })
        .multiunzip();

    quote!(
        use std::str::FromStr;
        use tl::VDom;
        impl SearchSelectors for #struct_name {
            fn search(dom: &VDom) -> Option<Self> {
                let parser = dom.parser();
                #(
                    let #name = FromStr::from_str(dom.query_selector(#sel)?.next()?.get(parser)?#output.as_ref()).ok()?;
                )*
                Some(Self { #(#name),* })
            }
        }
    ).into()
}
