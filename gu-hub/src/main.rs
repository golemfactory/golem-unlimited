extern crate futures;
extern crate tokio;

extern crate actix;
extern crate actix_web;
extern crate clap;
extern crate gu_actix;
extern crate gu_p2p;
extern crate gu_persist;
extern crate gu_lan;
extern crate tokio_uds;

extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

extern crate directories;

extern crate mdns;
extern crate rand;
extern crate gu_base;

extern crate env_logger;

use clap::App;
use gu_base::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

mod lan;
mod server;

fn main() {

    GuApp(||App::new("Golem Unlimited")
        .version(VERSION))
        .run( LogModule
            .chain(gu_persist::config::ConfigModule)
            .chain(lan::LanModule)
            .chain(server::ServerModule)
            .chain(CompleteModule::new()));

    /*
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )

        .subcommand(server::clap_declare())
        .subcommand(lan::clap_declare())
        .subcommand(SubCommand::with_name("status"))
        .

    if ::std::env::var("RUST_LOG").is_err() {
        ::std::env::set_var("RUST_LOG", "*=info,gu_p2p=debug,gu_provider=debug,gu_hub=debug")
    }
    env_logger::init();
    */
    debug!("debug");


    //server::clap_match(&matches);
    //lan::clap_match(&matches);
}
