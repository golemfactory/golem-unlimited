extern crate gu_actix;
extern crate gu_base;
extern crate gu_event_bus;
extern crate gu_ethkey;
extern crate gu_hardware;
extern crate gu_lan;
extern crate gu_net;
extern crate gu_persist;

extern crate serde;
extern crate serde_json;

extern crate actix;
extern crate actix_web;
extern crate futures;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

#[macro_use]
extern crate failure;

#[macro_use]
extern crate actix_derive;

#[macro_use]
extern crate prettytable;

extern crate bytes;
extern crate clap;
extern crate mdns;
extern crate rand;
extern crate semver;
extern crate sha1;
extern crate zip;

use clap::App;
use gu_base::*;
use gu_persist::daemon_module;

const VERSION: &str = env!("CARGO_PKG_VERSION");

mod peer;
mod plugins;
mod proxy_service;
mod server;
mod sessions;

fn main() {
    GuApp(|| App::new("Golem Unlimited").version(VERSION)).run(
        LogModule
            .chain(AutocompleteModule::new())
            .chain(gu_persist::config::ConfigModule::new())
            .chain(gu_lan::module::LanModule::module())
            .chain(plugins::PluginModule::new())
            .chain(sessions::SessionsModule::default())
            .chain(proxy_service::module())
            .chain(peer::PeerModule::new())
            .chain(gu_hardware::module())
            .chain(daemon_module::DaemonModule::hub())
            .chain(server::ServerModule::new()),
    );
}
