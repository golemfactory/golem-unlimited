extern crate futures;
extern crate tokio;

extern crate actix;
extern crate actix_web;
extern crate clap;
extern crate gu_p2p;
extern crate tokio_uds;

extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate error_chain;
extern crate directories;

use clap::{App, Arg, SubCommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");

mod config;
mod lan;
mod server;

fn main() {
    let matches = App::new("Golem Unlimited HUB")
        .version(VERSION)
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .arg(
            Arg::with_name("config-dir")
                .short("c")
                .takes_value(true)
                .value_name("PATH")
                .help("config dir path"),
        )
        .subcommand(server::clap_declare())
        .subcommand(lan::clap_declare())
        .subcommand(SubCommand::with_name("status"))
        .get_matches();

    if let Some(path) = matches.value_of("config") {}

    server::clap_match(&matches);
    lan::clap_match(&matches);
}
