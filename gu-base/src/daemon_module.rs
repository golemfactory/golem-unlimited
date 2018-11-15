use std::path::{Path, PathBuf};
use {
    daemon::{DaemonProcess, ProcessStatus},
    App, ArgMatches, Module, SubCommand,
};

enum GuServer {
    Hub,
    Provider,
}

impl GuServer {
    pub fn name(&self) -> &'static str {
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
    work_dir: PathBuf,
}

impl DaemonModule {
    pub fn hub<P: AsRef<Path>>(work_dir: P) -> Self {
        DaemonModule {
            server: GuServer::Hub,
            command: DaemonCommand::None,
            run: false,
            work_dir: work_dir.as_ref().into(),
        }
    }

    pub fn provider<P: AsRef<Path>>(work_dir: P) -> Self {
        DaemonModule {
            server: GuServer::Provider,
            command: DaemonCommand::None,
            run: false,
            work_dir: work_dir.as_ref().into(),
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
        let status = SubCommand::with_name("status").about("Checks whether server is running");

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

        let process = DaemonProcess::create(self.server.name(), &self.work_dir);

        match self.command {
            DaemonCommand::Run => match process.run_normally() {
                Ok(()) => self.run = true,
                Err(e) => println!("{}", e),
            },
            DaemonCommand::Start => match process.daemonize() {
                Ok(true) => self.run = true,
                Err(e) => println!("{}", e),
                _ => (),
            },
            DaemonCommand::Stop => {
                let _ = process.stop().map_err(|e| println!("{}", e));
            }
            DaemonCommand::Status => {
                let _ =
                    process
                        .status()
                        .map_err(|e| println!("{}", e))
                        .map(|status| match status {
                            ProcessStatus::Running(pid) => {
                                println!("Process is running (pid: {})", pid)
                            }
                            ProcessStatus::Stopped => println!("Process is not running"),
                        });
            }
            _ => (),
        }

        match self.command {
            DaemonCommand::None => false,
            _ => true,
        }
    }
}
