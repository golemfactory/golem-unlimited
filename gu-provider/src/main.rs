extern crate actix;
extern crate actix_web;
extern crate bytes;
extern crate clap;
#[macro_use]
extern crate error_chain;
extern crate flate2;
extern crate futures;
extern crate gu_actix;
extern crate gu_base;
extern crate gu_ethkey;
extern crate gu_hardware;
extern crate gu_lan;
extern crate gu_net;
extern crate gu_persist;
#[macro_use]
extern crate log;
extern crate mdns;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate gu_envman_api;
extern crate tar;
extern crate uuid;

use clap::App;
use gu_base::*;
use gu_persist::daemon_module;

pub mod envman;
mod hdman;
mod id;
mod provision;
mod server;
mod status;
mod sync_exec;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    GuApp(|| App::new("Golem Unlimited Provider").version(VERSION)).run(
        LogModule
            .chain(AutocompleteModule::new())
            .chain(gu_persist::config::ConfigModule::new())
            .chain(gu_lan::module::LanModule::module())
            .chain(gu_hardware::module())
            .chain(status::module())
            .chain(daemon_module::DaemonModule::provider())
            .chain(server::ServerModule::new()),
    );
}
