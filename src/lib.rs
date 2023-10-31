#![feature(async_fn_in_trait)]

mod client;
mod context;
mod error;
mod handler;
mod response;
mod website;

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
