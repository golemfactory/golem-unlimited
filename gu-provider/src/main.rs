extern crate futures;
extern crate tokio;

extern crate actix;
extern crate actix_web;
extern crate clap;
extern crate gu_base;
extern crate gu_actix;
extern crate gu_p2p;
extern crate gu_persist;
//extern crate gu_lan;

extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

//#[macro_use]
//extern crate error_chain;
extern crate directories;

extern crate env_logger;
#[macro_use]
extern crate log;

extern crate rand;
extern crate uuid;

mod hdman;
mod server;

const VERSION: &str = env!("CARGO_PKG_VERSION");

use clap::App;
use gu_base::*;

fn main() {
    GuApp(||App::new("Golem Unlimited Provider")
        .version(VERSION))
        .run( LogModule
            .chain(gu_persist::config::ConfigModule::new())
//            .chain(lan::LanModule)
            .chain(server::ServerModule::new())
            .chain(AutocompleteModule::new()));
}
