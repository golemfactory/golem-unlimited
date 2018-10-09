extern crate actix;
extern crate actix_web;
extern crate bytes;
extern crate clap;
extern crate directories;
extern crate env_logger;
#[macro_use]
extern crate error_chain;
extern crate flate2;
extern crate futures;
extern crate gu_actix;
extern crate gu_base;
extern crate gu_ethkey;
extern crate gu_hardware;
extern crate gu_lan;
extern crate gu_p2p;
extern crate gu_persist;
#[macro_use]
extern crate log;
extern crate mdns;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tar;
extern crate tokio;
extern crate uuid;

use clap::App;
use gu_base::*;

mod hdman;
mod id;
mod provision;
mod server;
mod status;
mod sync_exec;
mod write_to;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    GuApp(|| App::new("Golem Unlimited Provider").version(VERSION)).run(
        LogModule
            .chain(AutocompleteModule::new())
            .chain(gu_persist::config::ConfigModule::new())
            .chain(gu_lan::module::LanModule::module())
            .chain(gu_hardware::module())
            .chain(status::module())
            .chain(server::ServerModule::new()),
    );
}
