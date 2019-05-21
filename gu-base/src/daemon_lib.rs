use std::path::{Path, PathBuf};

use clap::{Arg,AppSettings};

use {
    daemon::{DaemonProcess, ProcessStatus},
    App, ArgMatches, SubCommand,
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

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum DaemonCommand {
    None,
    Run,
    Start,
    Stop,
    Status,
}

pub struct DaemonHandler {
    server: GuServer,
    command: DaemonCommand,
    work_dir: PathBuf,
    run_with_user_priviledges: bool,
}

impl DaemonHandler {
    pub fn hub<P: AsRef<Path>>(command: DaemonCommand, work_dir: P) -> Self {
        DaemonHandler {
            server: GuServer::Hub,
            command,
            work_dir: work_dir.as_ref().into(),
            run_with_user_priviledges: false,
        }
    }

    pub fn provider<P: AsRef<Path>>(command: DaemonCommand, work_dir: P, run_with_user_priviledges: bool) -> Self {
        DaemonHandler {
            server: GuServer::Provider,
            command,
            work_dir: work_dir.as_ref().into(),
            run_with_user_priviledges: run_with_user_priviledges,
        }
    }

    pub fn subcommand<'a, 'b>() -> App<'a, 'b> {
        let run = SubCommand::with_name("run").about("Run server in foreground");
        let start = SubCommand::with_name("start").about("Run server in background");
        let stop = SubCommand::with_name("stop").about("Stop currently running server");
        let status = SubCommand::with_name("status").about("Checks whether server is running");

        SubCommand::with_name("server")
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .about("Runs, gets status or stops a server on this machine")
            .subcommands(vec![run, start, stop, status])
            .arg(
                Arg::with_name("local")
                    .long("local")
                    .help("Local server (without special privileges)"),
            )
    }

    pub fn consume(matches: &ArgMatches) -> DaemonCommand {
        if let Some(m) = matches.subcommand_matches("server") {
            match m.subcommand_name() {
                Some("run") => DaemonCommand::Run,
                Some("start") => DaemonCommand::Start,
                Some("stop") => DaemonCommand::Stop,
                Some("status") => DaemonCommand::Status,
                _ => DaemonCommand::None,
            }
        } else {
            DaemonCommand::None
        }
    }

    pub fn run(&self) -> bool {
        let process = DaemonProcess::create(self.server.name(), &self.work_dir);

        match self.command {
            DaemonCommand::Run => match process.run_normally() {
                Ok(()) => return true,
                Err(e) => println!("{}", e),
            },
            DaemonCommand::Start => match process.daemonize() {
                Ok(true) => return true,
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

        false
    }
}
