extern crate futures;

extern crate actix;
extern crate actix_web;
extern crate clap;
extern crate gu_actix;
extern crate gu_event_bus;
extern crate gu_lan;
extern crate gu_p2p;
extern crate gu_persist;

extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

#[macro_use]
extern crate failure;

extern crate bytes;
extern crate gu_base;
extern crate mdns;
extern crate rand;
extern crate semver;
extern crate zip;

#[macro_use]
extern crate prettytable;

extern crate gu_hardware;

use clap::App;
use gu_base::*;
use gu_persist::daemon_module;

const VERSION: &str = env!("CARGO_PKG_VERSION");

mod peer;
mod plugins;
mod proxy_service;
mod server;

fn main() {
    GuApp(|| App::new("Golem Unlimited").version(VERSION)).run(
        LogModule
            .chain(AutocompleteModule::new())
            .chain(gu_persist::config::ConfigModule::new())
            .chain(gu_lan::module::LanModule::module())
            .chain(plugins::PluginModule::new())
            .chain(proxy_service::module())
            .chain(peer::PeerModule::new())
            .chain(gu_hardware::module())
            .chain(daemon_module::DaemonModule::hub())
            .chain(server::ServerModule::new()),
    );
}
