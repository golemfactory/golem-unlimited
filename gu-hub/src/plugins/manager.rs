use std::{
    collections::HashMap,
    fmt,
    fs::{self, remove_file, DirBuilder},
    io::{BufReader, Cursor},
    path::PathBuf,
};

use actix::{Actor, Context, Handler, Message, MessageResult, Supervised, SystemService};
use bytes::Bytes;
use log::{error, info, warn};
use semver::Version;

use gu_event_bus::post_event;
use gu_persist::config::ConfigModule;

use super::{
    parser::{BytesPluginParser, PluginParser, ZipParser},
    plugin::{
        DirectoryHandler, Plugin, PluginEvent, PluginHandler, PluginInfo, PluginStatus, ZipHandler,
    },
    rest_result::InstallQueryResult,
};

#[derive(Debug)]
pub struct PluginManager {
    /// version of currently running app
    gu_version: Version,
    /// map from a name of plugin into the plugin
    plugins: HashMap<String, Plugin>,
    /// directory containing plugin files
    directory: PathBuf,
}

impl Default for PluginManager {
    fn default() -> Self {
        let gu_version = Version::parse(env!("CARGO_PKG_VERSION"))
            .expect("Failed to run UI Plugin Manager:\nCouldn't parse crate version");

        let plugins_dir = ConfigModule::new().work_dir().join("plugins");
        info!("Plugins dir: {:?}", &plugins_dir);

        Self {
            gu_version,
            plugins: HashMap::new(),
            directory: plugins_dir,
        }
    }
}

impl PluginManager {
    fn install_plugin<T: 'static + PluginHandler>(&mut self, handler: T) -> InstallQueryResult {
        use super::rest_result::InstallQueryResult::*;

        let mut plugin = Plugin::new(handler);
        plugin.activate();

        plugin
            .metadata()
            .map_err(|a| InvalidMetadata(a))
            .map(|meta| {
                let event_path = format!("/plugins/{}", meta.name());
                post_event(event_path, PluginEvent::New(meta.clone()));
                match self.plugins.insert(meta.name().to_string(), plugin) {
                    None => Installed,
                    Some(_a) => Overwritten,
                }
            })
            .unwrap_or_else(|e| e)
    }

    fn uninstall_plugin(&mut self, name: &String) {
        let prev = self.plugins.remove(name);
        if prev.is_some() {
            let event_path = format!("/plugins/{}", name);
            post_event(event_path, PluginEvent::Drop(name.clone()));
        }

        // TODO: I would prefer some clear function in Plugin trait instead of this
        let file = self.directory.join(name);
        let _ = remove_file(file).map_err(|_| format!("Cannot remove plugin file {:?}", name));
    }

    fn load_zip(&mut self, name: &str) -> InstallQueryResult {
        let path = self.directory.join(name.to_string());
        ZipHandler::new(&path, self.gu_version.clone())
            .map_err(|e| InstallQueryResult::InvalidFile(e))
            .map(|handler| self.install_plugin(handler))
            .unwrap_or_else(|e| e)
    }

    fn save_plugin_file(&self, name: &str, bytes: &[u8]) -> Result<(), InstallQueryResult> {
        use self::InstallQueryResult::*;
        use std::path::Path;

        let path = self.directory.join(name.to_string());
        println!("{:?}", &path);
        if Path::new(&path).exists() {
            return Err(FileAlreadyExists);
        }

        fs::write(path, bytes)
            .map(|_| ())
            .map_err(|e| InvalidFile(e.to_string()))
    }

    /// Startup-only function for plugins loading
    fn reload_plugins(&mut self) {
        self.plugins.clear();

        let dir_res = fs::read_dir(&self.directory);
        match dir_res {
            Ok(dir) => {
                for plug_pack in dir {
                    let res = plug_pack
                        .map_err(|e| e.to_string())
                        .map(|pack| {
                            self.load_zip(pack.path().to_str().expect("Invalid path of plugin"))
                        })
                        .map(|a| format!("{:?}", a))
                        .unwrap_or_else(|e| e);

                    warn!("{:?}", res);
                }
            }
            Err(_) => error!("Cannot open {:?}.", &self.directory),
        }
    }

    fn plugin(&self, name: &str) -> Result<&Plugin, String> {
        self.plugins
            .get(name)
            .ok_or(format!("Cannot find {} plugin", name))
    }

    fn plugin_mut(&mut self, name: &str) -> Result<&mut Plugin, String> {
        self.plugins
            .get_mut(name)
            .ok_or(format!("Cannot find {} plugin", name))
    }
}

impl Supervised for PluginManager {}
impl SystemService for PluginManager {}

impl Actor for PluginManager {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        match DirBuilder::new().recursive(true).create(&self.directory) {
            Ok(_) => (),
            Err(e) => error!("Cannot create plugin dir ({})", e),
        }

        self.reload_plugins();
    }
}

/// LIST PLUGINS
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
            let _ = plugin
                .info()
                .map(|info| vec.push(info))
                .map_err(|e| warn!("Cannot get info: {}", e));
        }
        MessageResult(vec)
    }
}

/// GET PLUGIN FILE
#[derive(Debug)]
pub struct PluginFile {
    pub plugin: String,
    pub path: PathBuf,
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
        MessageResult(self.plugin(&msg.plugin).and_then(|plug| {
            plug.file(&msg.path)
                .map_err(|e| format!("Cannot find {}, {:?} file", e, msg.path))
        }))
    }
}

/// INSTALL PLUGIN
#[derive(Debug)]
pub struct InstallPlugin {
    pub bytes: Cursor<Bytes>,
}

impl Message for InstallPlugin {
    type Result = InstallQueryResult;
}

impl Handler<InstallPlugin> for PluginManager {
    type Result = MessageResult<InstallPlugin>;

    fn handle(
        &mut self,
        msg: InstallPlugin,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<InstallPlugin>>::Result {
        use self::InstallQueryResult::*;

        MessageResult(
            ZipParser::<BufReader<Cursor<Bytes>>>::from_bytes(msg.bytes.clone())
                .map_err(|a| InvalidFile(a))
                .and_then(|mut parser| {
                    parser
                        .validate_and_load_metadata(self.gu_version.clone())
                        .map_err(|e| InvalidMetadata(e))
                })
                .and_then(|metadata| {
                    let name = metadata.name();
                    self.save_plugin_file(name, msg.bytes.into_inner().as_ref())
                        .map(|_| self.load_zip(&name.to_string()))
                })
                .unwrap_or_else(|a| a),
        )
    }
}

/// CHANGE STATE
#[derive(Debug, Clone)]
pub enum QueriedStatus {
    Activate,
    Inactivate,
    Uninstall,
}

impl fmt::Display for QueriedStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let lowercase = format!("{:?}", self).to_ascii_lowercase();
        write!(f, "{}", lowercase)
    }
}

#[derive(Debug)]
pub struct ChangePluginState {
    pub plugin: String,
    pub state: QueriedStatus,
}

impl Message for ChangePluginState {
    type Result = Result<Option<PluginStatus>, String>;
}

impl Handler<ChangePluginState> for PluginManager {
    type Result = Result<Option<PluginStatus>, String>;

    fn handle(
        &mut self,
        msg: ChangePluginState,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<ChangePluginState>>::Result {
        let previous: Option<PluginStatus> =
            self.plugin(&msg.plugin).map(|plug| plug.status()).ok();

        let _ = match msg.state.clone() {
            QueriedStatus::Uninstall => Ok(self.uninstall_plugin(&msg.plugin)),

            _ => self
                .plugin_mut(&msg.plugin)
                .map(|plug| match msg.state.clone() {
                    QueriedStatus::Activate => plug.activate(),
                    QueriedStatus::Inactivate => plug.inactivate(),
                    _ => unreachable!(),
                }),
        };

        Ok(previous)
    }
}

/// DEV MODE
#[derive(Debug)]
pub struct InstallDevPlugin {
    pub path: PathBuf,
}

impl Message for InstallDevPlugin {
    type Result = InstallQueryResult;
}

impl Handler<InstallDevPlugin> for PluginManager {
    type Result = MessageResult<InstallDevPlugin>;

    fn handle(
        &mut self,
        msg: InstallDevPlugin,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<InstallDevPlugin>>::Result {
        use self::InstallQueryResult::*;

        let res = DirectoryHandler::new(msg.path)
            .map_err(|_| InvalidPath)
            .map(|handler| self.install_plugin(handler))
            .unwrap_or_else(|e| e);

        MessageResult(res)
    }
}
