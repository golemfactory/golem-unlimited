use actix::Actor;
use actix::Context;
use actix::Handler;
use actix::Message;
use actix::MessageResult;
use actix::Supervised;
use actix::SystemService;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use actix_web::Responder;
use bytes::Bytes;
use gu_persist::config::ConfigModule;
use plugins::parser::BytesPluginParser;
use plugins::parser::PluginParser;
use plugins::parser::ZipParser;
use plugins::plugin::DirectoryHandler;
use plugins::plugin::Plugin;
use plugins::plugin::PluginHandler;
use plugins::plugin::PluginInfo;
use plugins::plugin::PluginStatus;
use plugins::plugin::ZipHandler;
use semver::Version;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::fs::remove_file;
use std::fs::DirBuilder;
use std::fs::File;
use std::io::BufReader;
use std::io::Cursor;
use std::path::PathBuf;
use plugins::rest_result::InstallQueryResult;

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

        info!(
            "Plugins dir: {:?}",
            ConfigModule::new().work_dir().join("plugins")
        );

        Self {
            gu_version,
            plugins: HashMap::new(),
            directory: ConfigModule::new().work_dir().join("plugins"),
        }
    }
}

impl PluginManager {
    fn install_plugin<T: 'static + PluginHandler>(&mut self, handler: T) -> InstallQueryResult {
        use plugins::rest_result::InstallQueryResult::*;

        let mut plugin = Plugin::new(handler);
        plugin.activate();

        plugin
            .metadata()
            .map_err(|a| InvalidMetadata(a))
            .map(
                |meta| match self.plugins.insert(meta.name().to_string(), plugin) {
                    None => Installed,
                    Some(a) => Overwritten,
                },
            ).unwrap_or_else(|e| e)
    }

    fn uninstall_plugin(&mut self, name: &String) {
        self.plugins.remove(name);

        // TODO: I would prefer some clear function in Plugin trait instead of this
        let mut file = self.directory.clone();
        remove_file(file);
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

        let path = self.directory.with_file_name(zip_name(name.to_string()));
        if fs::File::open(path.clone()).is_ok() {
            return Err(FileAlreadyExists);
        }

        fs::write(path, bytes)
            .map(|_| ())
            .map_err(|e| InvalidFile(e.to_string()))
    }

    /// Startup-only function for plugins loading
    fn reload_plugins(&mut self) {
        self.plugins.clear();

        let dir = fs::read_dir(&self.directory).expect(&format!(
            "Cannot read plugins directory: {:?}",
            self.directory
        ));

        for plug_pack in dir {
            let res = plug_pack
                .map_err(|e| e.to_string())
                .map(|pack| self.load_zip(pack.path().to_str().expect("Invalid path of plugin")))
                .map(|a| format!("{:?}", a))
                .unwrap_or_else(|e| e);

            warn!("{:?}", res);
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
        DirBuilder::new()
            .recursive(true)
            .create(&self.directory)
            .unwrap();

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

fn zip_name(mut s: String) -> String {
    s.push_str(".zip");
    s
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
                }).and_then(|metadata| {
                    let name = metadata.name();
                    self.save_plugin_file(name, msg.bytes.into_inner().as_ref())
                        .map(|_| self.load_zip(&zip_name(name.to_string())))
                }).unwrap_or_else(|a| a),
        )
    }
}

/// (IN)ACTIVATE PLUGIN
#[derive(Debug, Clone)]
pub enum QueriedStatus {
    Activate,
    Inactivate,
    Uninstall,
    LogError(String),
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
        let mut previous: Option<PluginStatus> =
            self.plugin(&msg.plugin).map(|plug| plug.status()).ok();
        match msg.state.clone() {
            QueriedStatus::Uninstall => Ok(self.uninstall_plugin(&msg.plugin)),

            o => self
                .plugin_mut(&msg.plugin)
                .map(|plug| match msg.state.clone() {
                    QueriedStatus::Activate => plug.activate(),
                    QueriedStatus::Inactivate => plug.inactivate(),
                    QueriedStatus::LogError(s) => plug.log_error(s),
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
