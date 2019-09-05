#[cfg(feature = "win-service")]
#[macro_use]
extern crate windows_service;
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

mod exec_plugin;

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

fn inner() {
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
            .chain(exec_plugin::module())
            .chain(gu_lan::module::LanModule::module())
            .chain(gu_hardware::module())
            .chain(status::module())
            .chain(connect::module())
            .chain(permission::module())
            .chain(AutocompleteModule::new())
            .chain(server::ServerModule::new()),
    );
}

fn main() {
    #[cfg(feature = "win-service")]
    windows_inner();

    #[cfg(not(feature = "win-service"))]
    inner();
}

#[cfg(feature = "win-service")]
fn windows_inner() {
    use std::ffi::OsString;
    use windows_service::service::ServiceType;
    use windows_service::service_dispatcher;
    const SERVICE_NAME: &str = "gu-provider";
    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;
    define_windows_service!(ffi_service_main, my_service_main);

    fn my_service_main(_arguments: Vec<OsString>) {
        if let Err(e) = run_service() {
            log::error!("{:?}", e);
        }
    }

    fn run_service() -> windows_service::Result<()> {
        use std::time::Duration;
        use windows_service::service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        };
        use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                ServiceControl::Stop => {
                    actix::System::current().stop();
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };
        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
        })?;

        inner();

        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
        })?;

        Ok(())
    }

    let _ = service_dispatcher::start(SERVICE_NAME, ffi_service_main).map_err(|e| {
        log::error!("{:?}", e);
    });
}
