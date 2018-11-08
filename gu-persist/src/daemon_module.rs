use daemon::{daemonize_process, process_status, run_process_normally, stop_process, Process};
use gu_base::{App, ArgMatches, Module, SubCommand};

enum GuServer {
    Hub,
    Provider,
}

impl GuServer {
    #[allow(dead_code)]
    pub fn normal(&self) -> &'static str {
        match self {
            GuServer::Hub => "Hub",
            GuServer::Provider => "Provider",
        }
    }

    pub fn full(&self) -> &'static str {
        match self {
            GuServer::Hub => "gu-hub",
            GuServer::Provider => "gu-provider",
        }
    }
}

#[derive(PartialEq)]
enum DaemonCommand {
    None,
    Run,
    Start,
    Stop,
    Status,
}

pub struct DaemonModule {
    server: GuServer,
    command: DaemonCommand,
    run: bool,
}

impl DaemonModule {
    pub fn hub() -> Self {
        DaemonModule {
            server: GuServer::Hub,
            command: DaemonCommand::None,
            run: false,
        }
    }

    pub fn provider() -> Self {
        DaemonModule {
            server: GuServer::Provider,
            command: DaemonCommand::None,
            run: false,
        }
    }

    pub fn run(&self) -> bool {
        self.run
    }
}

impl Module for DaemonModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        let run = SubCommand::with_name("run").about("Run server in foreground");
        let start = SubCommand::with_name("start").about("Run server in background");
        let stop = SubCommand::with_name("stop").about("Stop currently running server");
        let status =
            SubCommand::with_name("status").about("Get status of currently running server");

        let command = SubCommand::with_name("server")
            .about("Server management")
            .subcommands(vec![run, start, stop, status]);

        app.subcommand(command)
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        if let Some(m) = matches.subcommand_matches("server") {
            self.command = match m.subcommand_name() {
                Some("run") => DaemonCommand::Run,
                Some("start") => DaemonCommand::Start,
                Some("stop") => DaemonCommand::Stop,
                Some("status") => DaemonCommand::Status,
                _ => DaemonCommand::None,
            }
        }

        let name = self.server.full();

        match self.command {
            DaemonCommand::Run => match run_process_normally(name) {
                Ok(()) => self.run = true,
                Err(e) => println!("{}", e),
            },
            DaemonCommand::Start => match daemonize_process(name) {
                Ok(true) => self.run = true,
                Err(e) => println!("{}", e),
                _ => (),
            },
            DaemonCommand::Stop => {
                let _ = stop_process(name).map_err(|e| println!("{}", e));
            }
            DaemonCommand::Status => {
                let _ = process_status(name)
                    .map_err(|e| println!("{}", e))
                    .map(|status| match status {
                        Process::Running(pid) => println!("Process is running (pid: {})", pid),
                        Process::Stopped => println!("Process is not running"),
                    });
            }
            _ => (),
        }

        false
    }
}
