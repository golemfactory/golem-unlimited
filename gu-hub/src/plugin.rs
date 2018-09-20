use actix_web::{self, http, HttpRequest, Responder, Scope};
use gu_base::{App, Arg, ArgMatches, Decorator, Module, SubCommand};
use gu_persist::config::ConfigModule;
use semver::Version;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use zip::ZipArchive;
use std::sync::Arc;
use std::io::Read;

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
        let gu_version = Version::parse(env!("CARGO_PKG_VERSION"))
            .expect("Failed to run UI Plugin Manager:\nCouldn't parse crate version");

        Self {
            gu_version,
            plugins: HashMap::new(),
            command: Command::None,
            directory: ConfigModule::new().work_dir().join("plugins"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
    files: HashMap<String, Arc<Vec<u8>>>,
    archive_name: String,
}

impl Plugin {
    pub fn new(archive_name: String, metadata: PluginMetadata) -> Self {
        Self {
            metadata,
            status: PluginStatus::Installed,
            files: HashMap::new(),
            archive_name,
        }
    }

    pub fn activate(&mut self) {
        // TODO: some action?
        self.status = PluginStatus::Active;
    }

    pub fn inactivate(&mut self) {
        self.files.clear();
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
    Install(PathBuf),
    Uninstall,
}

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
            self.command = match m.subcommand() {
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
            };
            true
        } else {
            false
        }
    }

    fn run<D: Decorator + Clone + 'static>(&self, _decorator: D) {
        match self.command {
            Command::None => (),
            Command::List => {
                println!("{:?}", self.plugins_list());
            }
            Command::Install(ref path) => {
                println!("{:?}", self.install_plugin(path));
            }
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

fn list_scope<S>(r: HttpRequest<S>) -> impl Responder {
    unimplemented!();
    ""
}

fn file_scope<S>(r: HttpRequest<S>) -> impl Responder {
    unimplemented!();
    ""
}

impl PluginManager {
    fn plugins_list(&self) -> Vec<PluginInfo> {
        let mut vec = Vec::new();
        for plugin in self.plugins.values() {
            vec.push(plugin.info())
        }
        vec
    }

    fn install_plugin(&self, path: &Path) -> Result<(), String> {
        let zip_name = path
            .file_name()
            .ok_or_else(|| format!("Cannot get zip archive name"))?;
        let metadata = extract_metadata(path)?;
        if metadata.version > self.gu_version {
            return Err(format!(
                "Too low gu-app version ({}). Required {}",
                self.gu_version, metadata.version
            ));
        }
        contains_app_js(&path, &metadata.name)?;

        fs::copy(&path, self.directory.join(zip_name))
            .and_then(|_| Ok(()))
            .map_err(|e| format!("Cannot copy zip archive: {:?}", e))
    }
}

fn open_archive(path: &Path) -> Result<ZipArchive<File>, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open archive: {:?}", e))?;
    ZipArchive::new(file).map_err(|e| format!("Cannot unzip file: {:?}", e))
}

fn extract_metadata(path: &Path) -> Result<PluginMetadata, String> {
    let mut archive = open_archive(path)?;

    let metadata_file = archive
        .by_name("gu-plugin.json")
        .map_err(|e| format!("Cannot read gu-plugin.json file: {:?}", e))?;

    serde_json::from_reader(metadata_file)
        .map_err(|e| format!("Cannot parse gu-plugin.json file: {:?}", e))
}

fn contains_app_js(path: &Path, name: &String) -> Result<(), String> {
    let mut archive = open_archive(path)?;
    let mut app_name = name.clone();
    app_name.push_str("/app.js");

    archive
        .by_name(app_name.as_ref())
        .map_err(|e| format!("Cannot read {} file: {:?}", app_name, e))?;

    Ok(())
}

fn load_archive(zip_path: &Path, app_name: &String) -> Result<HashMap<PathBuf, Arc<Vec<u8>>>, String> {
    let mut archive = open_archive(zip_path)?;
    let mut map = HashMap::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| warn!("Error during unzip: {:?}", e));

        if file.is_err() {
            continue
        }

        let mut file = file.unwrap();
        let out_path = file.sanitized_name();

        if out_path.starts_with(app_name) {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf);
            map.insert(out_path, Arc::new(buf));
        }
    }

    Ok(map)
}