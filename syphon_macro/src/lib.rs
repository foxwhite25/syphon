use std::collections::HashMap;

use itertools::Itertools;
use proc_macro::{Ident, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{
    parse_macro_input, DeriveInput, FieldsNamed, GenericArgument, Meta, PathArguments, PathSegment,
    Type,
};
#[derive(Debug)]
enum FieldType {
    String,
    VecOfString,
    OptionOfString,
}

impl FieldType {
    fn output(&self) -> proc_macro2::TokenStream {
        match self {
            FieldType::String => ".next()?;",
            FieldType::VecOfString => ".collect::<Vec<String>>();",
            FieldType::OptionOfString => ".next();",
        }
        .parse()
        .expect("failed to parse field type")
    }
}

fn generic_argument_eq(other: &PathSegment) -> Result<bool, &'static str> {
    let PathArguments::AngleBracketed(ref t) = other.arguments else {
        return Err("Not Anglebracket for Vec")
    };
    let GenericArgument::Type(t) = t.args.first().ok_or("Missing generic arguments")? else {
        return Err("Not Type in generic argument");
    };
    let Type::Path(path) = t else {
        return Err("Not path in genric argument");
    };
    Ok(path
        .path
        .segments
        .last()
        .ok_or("Empty Type path segments")?
        .ident
        .eq("String"))
}

impl TryFrom<Type> for FieldType {
    type Error = &'static str;

    fn try_from(value: Type) -> Result<Self, Self::Error> {
        let Type::Path(path) = value else {
            return Err("Type is not a path")
        };
        let t = path.path.segments.last().ok_or("Empty Type path segment")?;
        if t.ident.eq("Vec") && generic_argument_eq(t)? {
            return Ok(Self::VecOfString);
        }
        if t.ident.eq("Option") && generic_argument_eq(t)? {
            return Ok(Self::OptionOfString);
        }
        if t.ident.eq("String") {
            return Ok(Self::String);
        }
        Err("Currently only Support Vec<String> and String as type")
    }
}
#[derive(Debug)]
enum OutputVarience {
    Text,
    Attr(String),
}

impl TryFrom<HashMap<&str, &str>> for OutputVarience {
    type Error = &'static str;

    fn try_from(value: HashMap<&str, &str>) -> Result<Self, Self::Error> {
        if value.contains_key("text") {
            Ok(Self::Text)
        } else if value.contains_key("attr") {
            Ok(Self::Attr(value["attr"].to_string()))
        } else {
            Err("Either text or attr must be present")
        }
    }
}

impl OutputVarience {
    fn output(&self) -> proc_macro2::TokenStream {
        match self {
            OutputVarience::Text => r#".map(|x| x.text().collect::<Vec<_>>().join("\n"))"#,
            OutputVarience::Attr(_) => r#".filter_map(|x| x.attr("href").map(|s| s.to_string()))"#,
        }
        .parse()
        .expect("failed to parse output varience")
    }
}

#[derive(Debug)]
struct Field<'src> {
    name: &'src proc_macro2::Ident,
    field_type: FieldType,
    selector: String,
    varience: OutputVarience,
}

impl ToTokens for Field<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.append_all(self.output())
    }
}

impl<'src> Field<'src> {
    fn output(&self) -> proc_macro2::TokenStream {
        let name = self.name;
        let field = self.field_type.output();
        let varience = self.varience.output();
        let selector: &str = &self.selector.replace("\"", "");
        quote!(
            let #name = dom.select(&scraper::Selector::parse(#selector).ok()?)
                #varience
                #field
        )
    }
}
#[derive(Debug)]
struct Context<'src> {
    fields: Vec<Field<'src>>,
    struct_name: &'src proc_macro2::Ident,
}

impl<'src> Context<'src> {
    fn parse(fields: &'src FieldsNamed, struct_name: &'src proc_macro2::Ident) -> Self {
        let fields = fields
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
                    .unwrap_or_else(|| {
                        panic!("Unable to find attr \"select\" on field \"{name}\" ")
                    });

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

                let field_type = FieldType::try_from(x.ty.clone()).unwrap();
                let selector = attrs
                    .get("sel")
                    .unwrap_or_else(|| {
                        panic!("Unable to find sel in attribute for field \"{name}\"")
                    })
                    .to_string();
                let output_varience = OutputVarience::try_from(attrs).unwrap();

                Field {
                    name,
                    field_type,
                    selector,
                    varience: output_varience,
                }
            })
            .collect();
        Self {
            fields,
            struct_name,
        }
    }

    fn output(self) -> TokenStream {
        let name = self.fields.iter().map(|f| f.name).collect::<Vec<_>>();
        let field = self.fields;
        let struct_name = self.struct_name;
        quote!(
            use std::str::FromStr;
            impl SearchSelectors for #struct_name {
                fn search(dom: &scraper::Html) -> Option<Self> {
                    #(
                        #field;
                    )*
                    Some(Self { #(#name),* })
                }
            }
        )
        .into()
    }
}

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

    let ctx = Context::parse(fields, struct_name);
    ctx.output()
}
