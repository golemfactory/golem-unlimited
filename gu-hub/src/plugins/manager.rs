use actix::Actor;
use actix::Context;
use actix::Handler;
use actix::Message;
use actix::MessageResult;
use actix::Supervised;
use actix::SystemService;
use actix_web::dev::HttpResponseBuilder;
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
use std::fs;
use std::fs::DirBuilder;
use std::io::BufReader;
use std::io::Cursor;
use std::path::PathBuf;
use std::fs::File;
use std::fs::remove_file;

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
    fn install_plugin<T: 'static + PluginHandler>(&mut self, handler: T) -> Result<(), String> {
        let mut plugin = Plugin::new(handler);
        plugin.activate();

        println!("{:?}", plugin.metadata());

        plugin.metadata().and_then(|meta| {
            match self.plugins.insert(meta.name().to_string(), plugin) {
                None => Ok(()),
                Some(a) => Err(format!("Overwritten old ({:?}) module", a)),
            }
        })
    }

    fn uninstall_plugin(&mut self, name: &String) {
        self.plugins.remove(name);

        // TODO: I would prefer some clear function in Plugin trait instead of this
        let mut zip = self.directory.clone();
        zip.push(".zip");
        remove_file(zip);
    }

    fn load_zip(&mut self, name: &str) -> Result<(), String> {
        let path = self.directory.join(name.to_string());
        let handler = ZipHandler::new(&path, self.gu_version.clone())?;
        self.install_plugin(handler)
    }

    fn save_bytes_in_dir(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        fs::write(self.directory.join(zip_name(name.to_string())), bytes)
            .and_then(|_| Ok(()))
            .map_err(|e| format!("Cannot save file: {:?}", e))
    }

    fn reload_plugins(&mut self) -> Result<(), String> {
        self.plugins.clear();

        let dir = fs::read_dir(&self.directory)
            .map_err(|e| format!("Cannot read plugins directory: {:?}", e))?;

        for plug_pack in dir {
            plug_pack
                .map_err(|e| e.to_string())
                .and_then(|pack| ZipHandler::new(&pack.path(), self.gu_version.clone()))
                .and_then(|handler| self.install_plugin(handler))
                .map_err(|e| {
                    warn!(
                        "Cannot read file in plugins directory as zip archive: {:?}",
                        e
                    )
                });
        }

        Ok(())
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

        let _ = self.reload_plugins().map_err(|e| error!("{:?}", e));
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
    type Result = Result<(), String>;
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
        MessageResult(
            ZipParser::<BufReader<Cursor<Bytes>>>::from_bytes(msg.bytes.clone())
                .and_then(|mut parser| parser.validate_and_load_metadata(self.gu_version.clone()))
                .and_then(|metadata| {
                    let name = metadata.name();
                    self.save_bytes_in_dir(name, msg.bytes.into_inner().as_ref())
                        .and_then(|_| self.load_zip(&zip_name(name.to_string())))
                }).and_then(|_| Ok(())),
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
    type Result = Result<(), String>;
}

impl Handler<InstallDevPlugin> for PluginManager {
    type Result = MessageResult<InstallDevPlugin>;

    fn handle(
        &mut self,
        msg: InstallDevPlugin,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<InstallDevPlugin>>::Result {
        println!("{:?}", msg.path.clone());
        let plugin =
            DirectoryHandler::new(msg.path).and_then(|handler| self.install_plugin(handler));

        MessageResult(plugin.and_then(|_| Ok(())))
    }
}
