#![feature(async_fn_in_trait)]
#![feature(negative_impls)]
#![feature(auto_traits)]
#![allow(suspicious_auto_trait_impls)]
#![allow(clippy::new_ret_no_self)]
#![allow(clippy::type_complexity)]

pub mod client;
pub mod error;
pub mod handler;
pub mod next_action;
pub mod response;
pub mod website;

#[cfg(feature = "extractor")]
pub mod extractor;
