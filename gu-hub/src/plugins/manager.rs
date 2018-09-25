use actix::Actor;
use actix::Context;
use actix::Handler;
use actix::Message;
use actix::MessageResult;
use actix::Supervised;
use actix::SystemService;
use bytes::Bytes;
use gu_persist::config::ConfigModule;
use plugins::plugin::create_plugin_controller;
use plugins::plugin::PluginAPI;
use plugins::plugin::PluginInfo;
use semver::Version;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use plugins::parser::ZipParser;
use actix_web::dev::Resource;
use plugins::parser::PluginParser;

#[derive(Debug)]
pub struct PluginManager {
    /// version of currently running app
    gu_version: Version,
    /// map from a name of plugin into the plugin
    plugins: HashMap<String, Box<PluginAPI>>,
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
            directory: ConfigModule::new().work_dir().join("plugins"),
        }
    }
}

impl PluginManager {
    pub fn save_plugin(&self, path: &Path) -> Result<(), String> {
        let parser = create_plugin_controller(path, self.gu_version.clone())?;
        let metadata = parser.metadata();

        fs::copy(&path, self.directory.join(metadata.name()))
            .and_then(|_| Ok(()))
            .map_err(|e| format!("Cannot copy zip archive: {:?}", e))
    }

    fn reload_plugins(&mut self) -> Result<(), String> {
        self.plugins.clear();

        let dir = fs::read_dir(&self.directory)
            .map_err(|e| format!("Cannot read plugins directory: {:?}", e))?;

        for plug_pack in dir {
            let plug_pack =
                plug_pack.map_err(|e| format!("Cannot read plugin archive: {:?}", e))?;
            let plugin = create_plugin_controller(&plug_pack.path(), self.gu_version.clone());

            match plugin {
                Err(e) => warn!("Cannot load plugin: {:?}", e),
                Ok(mut plugin) => {
                    plugin.activate(self.directory.clone());
                    if let Some(old) = self.plugins.insert(plugin.name(), plugin) {
                        error!("Overwriting old ({:?}) module", old.name());
                    };
                }
            }
        }

        Ok(())
    }
}

impl Supervised for PluginManager {}
impl SystemService for PluginManager {}

impl Actor for PluginManager {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.reload_plugins().map_err(|e| error!("{:?}", e));
    }
}

pub struct ListPlugins;

impl Message for ListPlugins {
    type Result = Vec<PluginInfo>;
}

impl Handler<ListPlugins> for PluginManager {
    type Result = MessageResult<ListPlugins>;

    fn handle(
        &mut self,
        _msg: ListPlugins,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<ListPlugins>>::Result {
        let mut vec = Vec::new();
        for plugin in self.plugins.values() {
            vec.push(plugin.info())
        }
        MessageResult(vec)
    }
}

#[derive(Debug)]
pub struct PluginFile {
    pub plugin: String,
    pub path: String,
}

impl Message for PluginFile {
    type Result = Result<Vec<u8>, String>;
}

impl Handler<PluginFile> for PluginManager {
    type Result = MessageResult<PluginFile>;

    fn handle(
        &mut self,
        msg: PluginFile,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<PluginFile>>::Result {
        let plugin = self.plugins.get(&msg.plugin).unwrap();
        MessageResult(if plugin.is_active() {
            plugin
                .file(msg.path)
                .ok_or_else(|| "Page not found".to_string())
        } else {
            Err("Plugin is not active".to_string())
        })
    }
}

#[derive(Debug)]
pub struct InstallPlugin {
    pub message: Bytes,
}

impl Message for InstallPlugin {
    type Result = Result<(), String>;
}
/*
impl Handler<InstallPlugin> for PluginManager {
    type Result = MessageResult<InstallPlugin>;

    fn handle(
        &mut self,
        msg: InstallPlugin,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<InstallPlugin>>::Result {
        let mut parser: ZipParser<File> = ZipParser::new(PluginResource::Bytes(&msg.message));
        MessageResult(parser.validate_and_load_metadata(self.gu_version)
            .and_then(|metadata| {
                fs::copy(self.directory, self.directory.join(metadata.name()))
                    .and_then(|_| Ok(()))
                    .map_err(|e| format!("Cannot save archive: {:?}", e))
            }))
    }
}
*/