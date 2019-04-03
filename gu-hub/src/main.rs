extern crate ethkey;
extern crate gu_actix;
extern crate gu_base;
extern crate gu_event_bus;
extern crate gu_hardware;
extern crate gu_hdman;
extern crate gu_lan;
extern crate gu_model;
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

#[macro_use]
extern crate failure;

extern crate bytes;
extern crate clap;
extern crate hostname;
extern crate mdns;
extern crate semver;
extern crate sha1;
extern crate zip;

use gu_base::*;

/* TODO: replace with a macro (the code is the same as in the gu-hub/src/main.rs file) */
#[allow(dead_code)]
mod version {

    use gu_base::*;

    include!(concat!(env!("OUT_DIR"), "/version.rs"));

    struct Version;

    pub fn module() -> impl gu_base::Module {
        Version
    }

    impl gu_base::Module for Version {
        fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
            app.arg(
                Arg::with_name("ver-info")
                    .short("i")
                    .long("ver-info")
                    .help("Displays build details"),
            )
        }

        fn args_consume(&mut self, matches: &ArgMatches) -> bool {
            if matches.is_present("ver-info") {
                eprintln!("BUILD_TIMESTAMP  {}", VERGEN_BUILD_TIMESTAMP);
                eprintln!("COMMIT_DATE      {}", VERGEN_COMMIT_DATE);
                eprintln!("TARGET_TRIPLE    {}", VERGEN_TARGET_TRIPLE);
                eprintln!("SEMVER           {}", VERGEN_SEMVER);

                true
            } else {
                false
            }
        }
    }
}

const VERSION: &str = self::version::VERGEN_SEMVER_LIGHTWEIGHT;

mod peer;
mod plugins;
mod proxy_service;
mod server;
mod sessions;

fn main() {
    GuApp(|| {
        App::new("Golem Unlimited Hub")
            .setting(AppSettings::ArgRequiredElseHelp)
            .version(VERSION) /* TODO get port number from config */
            .after_help("The web UI is located at http://localhost:61622/app/index.html when the server is running.")
    })
    .run(
        LogModule
            .chain(gu_persist::config::ConfigModule::new())
            .chain(gu_lan::module::LanModule::module())
            .chain(gu_hardware::module())
            .chain(plugins::PluginModule::new())
            .chain(sessions::SessionsModule::default())
            .chain(proxy_service::module())
            .chain(peer::PeerModule::new())
            .chain(AutocompleteModule::new())
            .chain(server::ServerModule::new()),
    );
}
