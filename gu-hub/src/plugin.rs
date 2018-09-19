use actix::{Arbiter, System};
use gu_base::{App, ArgMatches, Decorator, Module, SubCommand};
use semver::Version;
use std::collections::HashMap;

#[derive(Debug)]
struct PluginManager {
    /// version of currently running app
    gu_version: Version,
    /// map from name of plugin into its metadata
    plugins: HashMap<String, PluginMetadata>,
    /// command that was chosen in command line
    command: Command,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            gu_version: Version::parse(env!("CARGO_PKG_VERSION"))
                .expect("Failed to run UI Plugin Manager:\nCouldn't parse crate version"),
            plugins: HashMap::new(),
            command: Command::None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PluginMetadata {
    /// plugin name
    name: String,
    /// plugin version
    version: Version,
    /// vendor
    author: String,
    /// optional plugin description
    description: Vec<String>,
    /// minimal required app version
    min_gu_version: Version,
    /// scripts to load on startup
    load: Vec<String>,
}

#[derive(Debug)]
enum Command {
    None,
    List,
    Install,
    Uninstall,
}

impl Module for PluginManager {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(SubCommand::with_name("plugin").subcommands(vec![
            SubCommand::with_name("install"),
            SubCommand::with_name("list"),
            SubCommand::with_name("uninstall"),
        ]))
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        if let Some(m) = matches.subcommand_matches("plugin") {
            self.command = match m.subcommand_name() {
                Some("list") => Command::List,
                Some("install") => Command::Install,
                Some("uninstall") => Command::Uninstall,
                None => Command::None,
                _ => return false,
            };
            true
        } else {
            false
        }
    }
}