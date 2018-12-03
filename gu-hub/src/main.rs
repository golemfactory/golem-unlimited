extern crate gu_actix;
extern crate gu_base;
extern crate gu_ethkey;
extern crate gu_event_bus;
extern crate gu_hardware;
extern crate gu_lan;
extern crate gu_net;
extern crate gu_persist;

extern crate serde;
extern crate serde_json;

extern crate actix;
extern crate actix_web;
extern crate chrono;
extern crate futures;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

#[macro_use]
extern crate actix_derive;

#[macro_use]
extern crate prettytable;

extern crate bytes;
extern crate clap;
extern crate mdns;
extern crate semver;
extern crate sha1;
extern crate zip;

use clap::App;
use gu_base::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

mod peer;
mod plugins;
mod proxy_service;
mod server;
mod sessions;

fn main() {
    let config_module = gu_persist::config::ConfigModule::new();

    GuApp(|| App::new("Golem Unlimited").version(VERSION)).run(
        LogModule
            .chain(daemon_module::DaemonModule::hub(
                config_module.work_dir().to_path_buf(),
            )).chain(server::ServerModule::new())
            .chain(config_module)
            .chain(gu_lan::module::LanModule::module())
            .chain(gu_hardware::module())
            .chain(plugins::PluginModule::new())
            .chain(sessions::SessionsModule::default())
            .chain(proxy_service::module())
            .chain(peer::PeerModule::new())
            .chain(AutocompleteModule::new()),
    );
}
