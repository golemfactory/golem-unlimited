use actix::{Arbiter, System};
use actix_web::{self, http, HttpRequest, Responder, Scope};
use gu_base::{App, ArgMatches, Decorator, Module, SubCommand};
use semver::Version;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
pub struct PluginManager {
    /// version of currently running app
    gu_version: Version,
    /// map from a name of plugin into the plugin
    plugins: HashMap<String, Plugin>,
    /// command that was chosen in command line
    command: Command,
    /// directory containing plugin files
    directory: PathBuf,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self {
            gu_version: Version::parse(env!("CARGO_PKG_VERSION"))
                .expect("Failed to run UI Plugin Manager:\nCouldn't parse crate version"),
            plugins: HashMap::new(),
            command: Command::None,
            // TODO: how to get information about the path?
            directory: "/home/hubert/IdeaProjects/golem-unlimited/gu-hub/webapp/plug".into(),
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
struct Plugin {
    metadata: PluginMetadata,
    status: PluginStatus,
}

impl Plugin {
    pub fn new(metadata: PluginMetadata) -> Self {
        Self {
            metadata,
            status: PluginStatus::Installed,
        }
    }

    pub fn activate(&mut self) {
        // TODO: some action?
        self.status = PluginStatus::Active;
    }

    pub fn inactivate(&mut self) {
        // TODO: some action?
        self.status = PluginStatus::Installed;
    }

    pub fn handle_error(&mut self) {
        // TODO: some action?
        self.status = PluginStatus::Error;
    }

    pub fn info(&self) -> PluginInfo {
        PluginInfo {
            name: self.metadata.name.clone(),
            version: self.metadata.version.clone(),
            status: self.status.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PluginInfo {
    name: String,
    version: Version,
    status: PluginStatus,
}

#[derive(Debug, Clone)]
enum PluginStatus {
    Active,
    Installed,
    Error,
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

    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        match self.command {
            Command::None => (),
            Command::List => {
                println!("{:?}", self.plugins_list());
            }
            Command::Install => (),
            Command::Uninstall => (),
        }
    }

    fn decorate_webapp<S: 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S> {
        app.scope("/app/plug", scope)
    }
}

fn scope<S: 'static>(scope: Scope<S>) -> Scope<S> {
    scope.route("", http::Method::GET, list_scope).route(
        "/{pluginName}/{fileName}",
        http::Method::GET,
        file_scope,
    )
}

impl PluginManager {
    fn plugins_list(&self) -> Vec<PluginInfo> {
        let mut vec = Vec::new();
        for plugin in self.plugins.values() {
            vec.push(plugin.info())
        }
        vec
    }
}

fn list_scope<S>(r: HttpRequest<S>) -> impl Responder {
    unimplemented!();
    ""
}

fn file_scope<S>(r: HttpRequest<S>) -> impl Responder {
    unimplemented!();
    ""
}
