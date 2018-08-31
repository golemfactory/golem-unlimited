#[macro_use]
extern crate error_chain;

extern crate actix;
extern crate dns_parser;
extern crate futures;
extern crate socket2;
extern crate tokio;
extern crate core;

pub mod resolve;
mod errors;