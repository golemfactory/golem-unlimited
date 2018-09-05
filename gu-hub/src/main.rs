extern crate futures;
extern crate tokio;

extern crate actix;
extern crate actix_web;
extern crate clap;
extern crate gu_p2p;
extern crate gu_persist;
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

mod lan;
mod server;

fn main() {
    let matches = App::new("Golem Unlimited Provider")
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
        .subcommand(SubCommand::with_name("status"))
        .get_matches();


}
