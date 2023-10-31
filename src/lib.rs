#![feature(async_fn_in_trait)]
#![feature(negative_impls)]
#![feature(auto_traits)]
#![allow(suspicious_auto_trait_impls)]

mod client;
mod context;
mod error;
mod handler;
mod next_action;
mod response;
mod website;

#[macro_use]
pub(crate) mod macros;

#[cfg(feature = "extractor")]
mod extractor;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
