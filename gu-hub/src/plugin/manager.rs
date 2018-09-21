use std::collections::HashMap;
use semver::Version;
use gu_persist::config::ConfigModule;
use std::path::Path;
use std::fs;
use std::path::PathBuf;
use plugin::plugin::PluginInfo;
use plugin::plugin::Plugin;
use plugin::zip::validate_and_load_metadata;

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

#[derive(Debug, Clone)]
pub enum Command {
    None,
    List,
    Install(PathBuf),
    Uninstall,
}

impl PluginManager {
    pub fn command(&self) -> Command {
        self.command.clone()
    }

    pub fn set_command(&mut self, command: Command) {
        self.command = command;
    }

    pub fn plugins_list(&self) -> Vec<PluginInfo> {
        let mut vec = Vec::new();
        for plugin in self.plugins.values() {
            vec.push(plugin.info())
        }
        vec
    }

    pub fn save_plugin(&self, path: &Path) -> Result<(), String> {
        let (zip_name, metadata) = validate_and_load_metadata(path, self.gu_version.clone())?;

        fs::copy(&path, self.directory.join(zip_name))
            .and_then(|_| Ok(()))
            .map_err(|e| format!("Cannot copy zip archive: {:?}", e))
    }

    fn reload_plugins(&mut self) -> Result<(), String> {
        self.plugins.clear();

        let dir = fs::read_dir(&self.directory)
            .map_err(|e| format!("Cannot read plugins directory: {:?}", e))?;

        for zip in dir {
            let zip = zip.map_err(|e| format!("Cannot read plugin archive: {:?}", e))?;
            let plugin = Plugin::load_archive(&zip.path(), self.gu_version.clone())
                .map_err(|e| format!("Cannot load plugin: {:?}", e))?;

            if let Some(old) = self.plugins.insert(plugin.name(), plugin) {
                error!("Overwriting old ({:?}) module", old);
            };
        }

        Ok(())
    }
}