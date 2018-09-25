use actix_web::{self, http, HttpRequest, Responder, Scope};
use gu_base::cli;
use gu_base::{App, Arg, ArgMatches, Decorator, Module, SubCommand};
use gu_persist::config::ConfigModule;
use plugins::zip::PluginParser;
use plugins::zip::ZipParser;
use prettytable::Table;
use semver::Version;
use semver::VersionReq;
use serde_json;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::fs::File;
use std::io::Read;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct PluginMetadata {
    /// plugin name
    name: String,
    /// plugin version
    version: Version,
    /// vendor
    author: String,
    /// optional plugin description
    description: Vec<String>,
    /// minimal required app version
    gu_version_req: VersionReq,
    /// scripts to load on startup
    load: Vec<String>,
}

impl PluginMetadata {
    pub fn proper_version(&self, version: &Version) -> bool {
        self.gu_version_req.matches(version)
    }

    pub fn version(&self) -> Version {
        self.version.clone()
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }
}

pub trait PluginAPI: Debug {
    fn name(&self) -> String;

    fn activate(&mut self, plugins_dir: PathBuf) -> Result<(), String>;

    fn inactivate(&mut self);

    fn handle_error(&mut self);

    fn info(&self) -> PluginInfo;

    fn file(&self, path: String) -> Option<Vec<u8>>;

    fn is_active(&self) -> bool;

    fn archive_name(&self) -> String;

    fn metadata(&self) -> PluginMetadata;
}

pub fn create_plugin_controller(
    path: &Path,
    gu_version: Version,
) -> Result<Box<PluginAPI>, String> {
    Plugin::<ZipParser>::load_metadata(path, gu_version)
}

#[derive(Debug)]
pub struct Plugin<T: PluginParser + 'static> {
    metadata: PluginMetadata,
    status: PluginStatus,
    files: HashMap<PathBuf, Vec<u8>>,
    archive_name: String,
    phantom: PhantomData<T>,
}

impl<T: PluginParser + 'static> Plugin<T> {
    fn load_metadata(path: &Path, gu_version: Version) -> Result<Box<PluginAPI>, String> {
        let (zip_name, metadata) = T::validate_and_load_metadata(path, gu_version)?;
        Ok(Self::new(zip_name, metadata))
    }

    fn new(archive_name: String, metadata: PluginMetadata) -> Box<PluginAPI> {
        Box::new(Self {
            metadata,
            status: PluginStatus::Installed,
            files: HashMap::new(),
            archive_name,
            phantom: PhantomData::<T>,
        })
    }
}

impl<T: PluginParser + 'static> PluginAPI for Plugin<T> {
    fn name(&self) -> String {
        self.metadata.name.clone()
    }

    fn activate(&mut self, plugins_dir: PathBuf) -> Result<(), String> {
        let plugin_path = plugins_dir.join(&self.archive_name);
        self.files = T::load_files(plugin_path.as_ref(), &self.metadata.name)?;
        self.status = PluginStatus::Active;
        Ok(())
    }

    fn inactivate(&mut self) {
        self.files.clear();
        self.status = PluginStatus::Installed;
    }

    fn handle_error(&mut self) {
        // TODO: some action?
        self.status = PluginStatus::Error;
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: self.metadata.name.clone(),
            version: self.metadata.version.clone(),
            status: self.status.clone(),
        }
    }

    fn file(&self, path: String) -> Option<Vec<u8>> {
        self.files.get(&PathBuf::from(path)).map(|arc| arc.clone())
    }

    fn is_active(&self) -> bool {
        self.status == PluginStatus::Active
    }

    fn archive_name(&self) -> String {
        self.archive_name.clone()
    }

    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    name: String,
    version: Version,
    status: PluginStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum PluginStatus {
    Active,
    Installed,
    Error,
}

impl fmt::Display for PluginStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub fn format_plugins_table(plugins: Vec<PluginInfo>) {
    cli::format_table(
        row!["Name", "Version", "Status"],
        || "No plugins installed",
        plugins.iter().map(|plugin| {
            row![
                plugin.name,
                plugin.version.to_string(),
                plugin.status.to_string(),
            ]
        }),
    )
}
