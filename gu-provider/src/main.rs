extern crate futures;

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

mod version {
    use gu_base::*;

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
                eprintln!("BUILD_TIMESTAMP  {}", env!("VERGEN_BUILD_TIMESTAMP"));
                eprintln!("COMMIT_DATE      {}", env!("VERGEN_COMMIT_DATE"));
                eprintln!("TARGET_TRIPLE    {}", env!("VERGEN_TARGET_TRIPLE"));
                eprintln!("SEMVER           {}", env!("VERGEN_SEMVER"));

                true
            } else {
                false
            }
        }
    }
}

const VERSION: &str = env!("VERGEN_SEMVER_LIGHTWEIGHT");

fn main() {
    let config_module = gu_persist::config::ConfigModule::new();

    GuApp(|| {
        App::new("Golem Unlimited Provider")
            .setting(AppSettings::ArgRequiredElseHelp)
            .version(VERSION)
    })
    .run(
        LogModule
            .chain(version::module())
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
