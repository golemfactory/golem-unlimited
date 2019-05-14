use std::path::PathBuf;

use actix_web;

use gu_base::{App, AppSettings, Arg, ArgMatches, Decorator, Module, SubCommand};

use super::{builder, manager::QueriedStatus, rest};

#[derive(Debug)]
pub struct PluginModule {
    command: Command,
}

#[derive(Debug, Clone)]
enum Command {
    None,
    List,
    Install(PathBuf),
    Dev(PathBuf),
    Uninstall(String),
    Activate(String),
    Inactivate(String),
    Build(builder::BuildPluginQuery),
}

impl PluginModule {
    pub fn new() -> Self {
        Self {
            command: Command::None,
        }
    }
}

impl Module for PluginModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        let plugin = Arg::with_name("PLUGIN")
            .help("Plugin name")
            .required(true)
            .index(1);

        let dir = Arg::with_name("DIR")
            .help("Specifies path to the plugin directory")
            .required(true)
            .index(1);

        let path = Arg::with_name("PATH")
            .help("path to the package")
            .required(true)
            .index(1);

        app.subcommand(
            SubCommand::with_name("plugin")
                .about("Manages web UI plugins (e.g. builds, installs, lists, starts, stops and uninstalls them)")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommands(vec![
                    SubCommand::with_name("install")
                        .about("Installs the plugin from the package")
                        .arg(path),
                    SubCommand::with_name("dev")
                        .about("Installs the plugin in a dev mode")
                        .arg(dir),
                    SubCommand::with_name("list").about("Lists currently installed plugins"),
                    SubCommand::with_name("start")
                        .about("Starts serving files by the plugin")
                        .arg(Arg::from(&plugin)),
                    SubCommand::with_name("stop")
                        .about("Stops serving files by the plugin")
                        .arg(Arg::from(&plugin)),
                    SubCommand::with_name("uninstall")
                        .about("Uninstalls the plugin")
                        .arg(Arg::from(&plugin)),
                    builder::subcommand(),
                ]),
        )
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        if let Some(m) = matches.subcommand_matches("plugin") {
            self.command = match m.subcommand() {
                ("list", Some(_)) => Command::List,
                ("install", Some(m)) => {
                    let tar_path = PathBuf::from(
                        m.value_of("PATH")
                            .expect("Lack of required `archive` argument"),
                    );
                    Command::Install(tar_path)
                }
                ("dev", Some(m)) => {
                    let dir_path = PathBuf::from(
                        m.value_of("DIR")
                            .expect("Lack of required `dir-path` argument"),
                    );
                    Command::Dev(dir_path)
                }
                ("uninstall", Some(m)) => {
                    let name = String::from(
                        m.value_of("PLUGIN")
                            .expect("Lack of required `plugin` argument"),
                    );
                    Command::Uninstall(name)
                }
                ("start", Some(m)) => {
                    let name = String::from(
                        m.value_of("PLUGIN")
                            .expect("Lack of required `plugin` argument"),
                    );
                    Command::Activate(name)
                }
                ("stop", Some(m)) => {
                    let name = String::from(
                        m.value_of("PLUGIN")
                            .expect("Lack of required `plugin` argument"),
                    );
                    Command::Inactivate(name)
                }
                ("build", Some(m)) => Command::Build(m.to_owned().into()),
                _ => Command::None,
            };
            match self.command {
                Command::None => false,
                _ => true,
            }
        } else {
            false
        }
    }

    fn run<D: Decorator + Clone + 'static>(&self, _decorator: D) {
        match self.command {
            Command::None => (),
            Command::List => rest::list_query(),
            Command::Install(ref path) => rest::install_query(path.into()),
            Command::Dev(ref path) => rest::dev_query(path.to_path_buf()),
            Command::Uninstall(ref name) => rest::uninstall_query(name.to_string()),
            Command::Activate(ref name) => {
                rest::status_query(name.to_string(), QueriedStatus::Activate)
            }
            Command::Inactivate(ref name) => {
                rest::status_query(name.to_string(), QueriedStatus::Inactivate)
            }
            Command::Build(ref obj) => builder::build_query(obj),
        }
    }

    fn decorate_webapp<S: 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S> {
        app.scope("/plug", rest::scope)
    }
}
