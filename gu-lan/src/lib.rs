#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate prettytable;

extern crate gu_actix;
extern crate gu_base;
extern crate gu_p2p;

extern crate bytes;
extern crate rand;
extern crate serde_json;
extern crate socket2;

extern crate actix;
extern crate actix_web;
extern crate clap;
extern crate dns_parser;
extern crate futures;
extern crate tokio;
extern crate tokio_codec;

pub mod actor;
mod codec;
pub mod continuous;
mod errors;
pub mod rest_client;
pub mod server;
pub mod service;

pub const LAN_ENDPOINT: u32 = 576411;
