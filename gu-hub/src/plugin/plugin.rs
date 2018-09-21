use actix_web::{self, http, HttpRequest, Responder, Scope};
use gu_base::{App, Arg, ArgMatches, Decorator, Module, SubCommand};
use gu_persist::config::ConfigModule;
use semver::VersionReq;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use zip::ZipArchive;
use prettytable::Table;
use gu_base::cli;
use std::fmt;
use plugin::zip::validate_and_load_metadata;
use plugin::zip::load_archive;
use semver::Version;

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug)]
pub struct Plugin {
    metadata: PluginMetadata,
    status: PluginStatus,
    files: HashMap<PathBuf, Arc<Vec<u8>>>,
    archive_name: String,
}

impl Plugin {
    pub fn name(&self) -> String {
        self.metadata.name.clone()
    }

    pub fn load_archive(path: &Path, gu_version: Version) -> Result<Self, String> {
        let (zip_name, metadata) = validate_and_load_metadata(path, gu_version)?;
        Ok(Self::new(zip_name, metadata))
    }

    pub fn new(archive_name: String, metadata: PluginMetadata) -> Self {
        Self {
            metadata,
            status: PluginStatus::Installed,
            files: HashMap::new(),
            archive_name,
        }
    }

    pub fn activate(&mut self, plugins_dir: PathBuf) -> Result<(), String> {
        let zip_path = plugins_dir.join(&self.archive_name);
        self.files = load_archive(zip_path.as_ref(), &self.metadata.name)?;
        self.status = PluginStatus::Active;
        Ok(())
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

impl fmt::Display for PluginStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}



pub fn format_plugins_table(plugins: Vec<PluginInfo>) {
    let mut table = Table::new();
    table.set_titles(row!["Name", "Version", "Status"]);
    for plugin in plugins {
        table.add_row(row![
            plugin.name,
            plugin.version.to_string(),
            plugin.status.to_string(),
        ]);
    }

    table.set_format(*cli::FORMAT_BASIC);
    table.printstd()
}
