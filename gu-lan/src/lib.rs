#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;

extern crate actix;
extern crate bytes;
extern crate dns_parser;
extern crate futures;
extern crate socket2;
extern crate tokio;
extern crate tokio_codec;

mod errors;
mod mdns_codec;
pub mod resolve_actor;
pub mod service;
