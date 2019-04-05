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
