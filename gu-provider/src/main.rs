extern crate actix;
#[macro_use]
extern crate actix_derive;
extern crate actix_web;
extern crate bytes;
extern crate clap;
#[macro_use]
extern crate error_chain;
extern crate flate2;
extern crate futures;
#[macro_use]
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
#[cfg(feature = "env-docker")]
extern crate async_docker;

extern crate gu_model;
#[macro_use]
extern crate serde_json;
extern crate tar;
extern crate uuid;
#[macro_use]
extern crate prettytable;

extern crate crossbeam_channel;
extern crate futures_cpupool;
extern crate tar_async;

use clap::App;
use gu_base::*;

mod connect;
mod deployment;
pub mod envman;
mod hdman;
mod id;
mod permission;
mod provision;
mod server;
mod status;
mod sync_exec;
mod sync_stream;
mod workspace;

#[cfg(feature = "env-docker")]
mod dockerman;

#[cfg(not(feature = "env-docker"))]
mod dockerman {
    pub use gu_base::empty::module;
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let config_module = gu_persist::config::ConfigModule::new();

    GuApp(|| App::new("Golem Unlimited Provider").version(VERSION)).run(
        LogModule
            .chain(config_module)
            .chain(dockerman::module())
            .chain(gu_lan::module::LanModule::module())
            .chain(gu_hardware::module())
            .chain(status::module())
            .chain(connect::module())
            .chain(permission::module())
            .chain(AutocompleteModule::new())
            .chain(server::ServerModule::new()),
    );
}
