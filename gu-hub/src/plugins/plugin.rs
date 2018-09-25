use gu_base::cli;
use plugins::parser::PluginParser;
use plugins::parser::ZipParser;
use semver::Version;
use semver::VersionReq;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::fs::File;
use plugins::parser::PathPluginParser;

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
    pub fn version(&self) -> Version {
        self.version.clone()
    }

    pub fn gu_version_req(&self) -> VersionReq {
        self.gu_version_req.clone()
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

    fn metadata(&self) -> PluginMetadata;
}

pub fn create_plugin_controller(
    path: &Path,
    gu_version: Version,
) -> Result<Box<PluginAPI>, String> {
    let parser = ZipParser::new(path)?;
    Plugin::<ZipParser<File>>::new(parser, gu_version)
}

#[derive(Debug)]
pub struct Plugin<T: PluginParser + 'static> {
    metadata: PluginMetadata,
    status: PluginStatus,
    files: HashMap<PathBuf, Vec<u8>>,
    parser: T,
}

impl<T: PathPluginParser + 'static> Plugin<T> {
    fn new(mut parser: T, gu_version: Version) -> Result<Box<PluginAPI>, String> {
        let metadata = parser.validate_and_load_metadata(gu_version)?;

        Ok(Box::new(Self {
            metadata,
            status: PluginStatus::Installed,
            files: HashMap::new(),
            parser,
        }))
    }
}

impl<T: PluginParser + 'static> PluginAPI for Plugin<T> {
    fn name(&self) -> String {
        self.metadata.name.clone()
    }

    fn activate(&mut self, plugins_dir: PathBuf) -> Result<(), String> {
        let plugin_path = plugins_dir.join(&self.metadata.name());
        self.files = self.parser.load_files(&self.metadata.name)?;
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
