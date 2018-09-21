use gu_base::Module;
use plugin::manager::PluginManager;
use gu_base::App;
use gu_base::SubCommand;
use gu_base::Arg;
use gu_base::ArgMatches;
use gu_base::Decorator;
use plugin::manager::Command;
use actix_web;
use plugin::rest::scope;
use std::path::PathBuf;
use plugin::plugin::format_plugins_table;

impl Module for PluginManager {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(SubCommand::with_name("plugin").subcommands(vec![
            SubCommand::with_name("install").arg(
                Arg::with_name("archive")
                    .takes_value(true)
                    .short("a")
                    .help("specifies path to archive")
                    .required(true)
            ),

            SubCommand::with_name("list"),

            SubCommand::with_name("uninstall"),
        ]))
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        if let Some(m) = matches.subcommand_matches("plugin") {
            self.set_command(match m.subcommand() {
                ("list", Some(_)) => Command::List,
                ("install", Some(m)) => {
                    let tar_path = PathBuf::from(
                        m.value_of("archive")
                            .expect("Lack of required `archive` argument"),
                    );
                    Command::Install(tar_path)
                }
                ("uninstall", Some(_)) => Command::Uninstall,
                ("", None) => Command::None,
                _ => return false,
            });
            true
        } else {
            false
        }
    }

    fn run<D: Decorator + Clone + 'static>(&self, _decorator: D) {
        match self.command() {
            Command::None => (),
            Command::List => {
                format_plugins_table(self.plugins_list());
            }
            Command::Install(ref path) => {
                println!("{:?}", self.save_plugin(path));
            }
            Command::Uninstall => {

            },
        }
    }

    fn decorate_webapp<S: 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S> {
        app.scope("/app/plug", scope)
    }
}