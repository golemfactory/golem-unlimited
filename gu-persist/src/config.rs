#![allow(proc_macro_derive_resolution_fallback)]

use std::{
    any::Any,
    borrow::Cow,
    collections::HashMap,
    marker::PhantomData,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use actix::{fut, prelude::*};
use directories::ProjectDirs;
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value as JsonValue};

use gu_actix::*;
use gu_base::{App, Arg, ArgMatches, Module};

pub use super::error::*;
use super::storage::{Fetch, Put};

type Storage = super::file_storage::FileStorage;

#[derive(Default)]
pub struct ConfigManager {
    storage: Option<Addr<Storage>>,
    cache: HashMap<&'static str, Box<Any + 'static>>,
}

impl ConfigManager {
    fn storage(&mut self) -> &Addr<Storage> {
        let config_dir = ConfigModule::new().config_dir();
        let storage = match self.storage.take() {
            Some(v) => v,
            None => SyncArbiter::start(1, move || Storage::from_path(&config_dir)),
        };
        self.storage = Some(storage);
        self.storage.as_ref().unwrap()
    }
}

impl Actor for ConfigManager {
    type Context = Context<Self>;
}

impl Supervised for ConfigManager {}
impl SystemService for ConfigManager {}

pub trait ConfigSection: Sized + HasSectionId + Default {
    fn from_json(json: JsonValue) -> Result<Self>;

    fn to_json(&self) -> Result<JsonValue>;
}

pub trait HasSectionId {
    const SECTION_ID: &'static str;
}

impl<T: HasSectionId + Serialize + for<'de> Deserialize<'de> + Default> ConfigSection for T {
    fn from_json(json: JsonValue) -> Result<Self> {
        Ok(serde_json::from_value(json)?)
    }

    fn to_json(&self) -> Result<JsonValue> {
        Ok(serde_json::to_value(self)?)
    }
}

pub struct GetConfig<T: ConfigSection>(PhantomData<T>);

impl<T: ConfigSection> GetConfig<T> {
    pub fn new() -> Self {
        GetConfig(PhantomData)
    }
}

impl<T: ConfigSection + 'static> Message for GetConfig<T> {
    type Result = Result<Arc<T>>;
}

#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct SetConfig<T: ConfigSection>(Arc<T>);

impl<T: ConfigSection> SetConfig<T> {
    pub fn new(inner: T) -> Self {
        SetConfig(Arc::new(inner))
    }
}

impl<T: ConfigSection + 'static + Send + Sync> Handler<GetConfig<T>> for ConfigManager {
    type Result = ActorResponse<ConfigManager, Arc<T>, Error>;

    fn handle(&mut self, _: GetConfig<T>, _ctx: &mut Self::Context) -> Self::Result {
        let key = T::SECTION_ID;

        if let Some(ref v) = self.cache.get(key) {
            let y = v.downcast_ref::<Arc<T>>();

            if let Some(v) = y {
                return ActorResponse::reply(Ok(v.clone()));
            }
        }

        ActorResponse::r#async(
            self.storage()
                .send(Fetch(Cow::Borrowed(T::SECTION_ID)))
                .flatten_fut()
                .into_actor(self)
                .and_then(|r, act, ctx| {
                    let v = match r {
                        None => Arc::new(T::default()),
                        Some(v) => {
                            let p: JsonValue = match serde_json::from_slice(v.as_ref()) {
                                Ok(v) => v,
                                Err(e) => return fut::Either::A(fut::err(e.into())),
                            };
                            Arc::new(T::from_json(p).unwrap())
                        }
                    };
                    fut::Either::B(
                        ctx.address()
                            .send(SetConfig(v.clone()))
                            .flatten_fut()
                            .and_then(move |_| Ok(v))
                            .into_actor(act),
                    )
                }),
        )
    }
}

macro_rules! async_try {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => return ActorResponse::reply(Err(e.into())),
        }
    };
}

impl<T: ConfigSection + 'static> Handler<SetConfig<T>> for ConfigManager {
    type Result = ActorResponse<ConfigManager, (), Error>;

    fn handle(&mut self, msg: SetConfig<T>, _ctx: &mut Self::Context) -> Self::Result {
        let k = Cow::Borrowed(T::SECTION_ID);
        let v = msg.0;
        let json = async_try!(v.to_json());
        let bytes = async_try!(serde_json::to_vec(&json));

        self.cache.insert(T::SECTION_ID, Box::new(v));

        ActorResponse::r#async(
            self.storage()
                .send(Put(k, bytes))
                .flatten_fut()
                .into_actor(self),
        )
    }
}

pub struct ConfigModule;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigPaths {
    work_dir: PathBuf,
    cache_dir: PathBuf,
    #[serde(skip)]
    config_dir: PathBuf,
    runtime_dir: PathBuf,
}

lazy_static! {
    static ref CONFIG_PATHS_LOCK: RwLock<ConfigPaths> = RwLock::new(ConfigPaths {
        work_dir: PathBuf::from("/var/lib/golemu/data/"),
        cache_dir: PathBuf::from("/var/cache/golemu/"),
        config_dir: PathBuf::from("/var/lib/golemu/conf/"),
        runtime_dir: PathBuf::from("/var/run/golemu/"),
    });
    static ref CONFIG_DIR_ENV_VAR_LOCK: RwLock<Option<PathBuf>> = RwLock::new(None);
}

fn create_app_dirs() -> std::io::Result<()> {
    let paths = CONFIG_PATHS_LOCK.read().unwrap();
    for dir in &[
        &paths.work_dir,
        &paths.cache_dir,
        &paths.config_dir,
        &paths.runtime_dir,
    ] {
        if !dir.exists() {
            std::fs::create_dir_all(&dir).map_err(|e| {
                error!("Cannot create {:?}.", dir);
                e
            })?
        }
    }
    Ok(())
}

fn set_config_path(config_path: PathBuf) {
    let mut unlocked_paths = CONFIG_PATHS_LOCK.write().unwrap();
    let dir_paths = config_path.clone().join("dir-paths.json");
    let join_if_relative = |path: PathBuf, config_dir: &PathBuf| {
        if path.is_relative() {
            config_dir.join(path)
        } else {
            path
        }
    };
    if let Ok(data) = std::fs::read_to_string(&dir_paths) {
        let config_paths: ConfigPaths =
            serde_json::from_str(&data).expect(&format!("Cannot deserialize {:?}", dir_paths));
        unlocked_paths.work_dir = join_if_relative(config_paths.work_dir, &config_path);
        unlocked_paths.cache_dir = join_if_relative(config_paths.cache_dir, &config_path);
        unlocked_paths.runtime_dir = join_if_relative(config_paths.runtime_dir, &config_path);
    } else {
        let default_content = serde_json::to_string_pretty(&*unlocked_paths).unwrap();
        error!(
            "Could not find {:?}; creating this file using defaults: {}.",
            &dir_paths, &default_content,
        );
        use std::{
            fs::{create_dir_all, File},
            io::Write,
        };
        if !config_path.exists() {
            let _ = create_dir_all(&config_path).map_err(|e| error!("{}", e));
        }
        let _ = File::create(dir_paths).and_then(|mut f| f.write_all(default_content.as_bytes()));
    }
    unlocked_paths.config_dir = config_path;
}

fn set_paths_local() {
    let dirs = ProjectDirs::from("network", "Golem", "Golem Unlimited").unwrap();
    let paths = ConfigPaths {
        work_dir: dirs.data_local_dir().to_path_buf().join("data"),
        cache_dir: dirs.cache_dir().to_path_buf(),
        config_dir: dirs.config_dir().to_path_buf(),
        runtime_dir: dirs.data_local_dir().to_path_buf().join("run"),
    };
    *CONFIG_PATHS_LOCK.write().unwrap() = paths;
}

impl ConfigModule {
    const KEYSTORE_FILE: &'static str = "keystore.json";

    pub fn new() -> Self {
        ConfigModule {}
    }

    /// TODO: for extracted sessions
    pub fn work_dir(&self) -> PathBuf {
        CONFIG_PATHS_LOCK.read().unwrap().work_dir.clone()
    }

    /// TODO: for downloaded images
    pub fn cache_dir(&self) -> PathBuf {
        CONFIG_PATHS_LOCK.read().unwrap().cache_dir.clone()
    }

    /// TODO: for configs and ethkeys
    pub fn config_dir(&self) -> PathBuf {
        CONFIG_PATHS_LOCK.read().unwrap().config_dir.clone()
    }

    pub fn runtime_dir(&self) -> PathBuf {
        CONFIG_PATHS_LOCK.read().unwrap().runtime_dir.clone()
    }

    pub fn keystore_path(&self) -> PathBuf {
        self.config_dir()
            .to_path_buf()
            .join(ConfigModule::KEYSTORE_FILE)
    }
}

impl Module for ConfigModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        *CONFIG_DIR_ENV_VAR_LOCK.write().unwrap() = match app.get_name() {
            "Golem Unlimited Provider" => std::env::var("GU_PROV_CONF_DIR").ok().map(PathBuf::from),
            "Golem Unlimited Hub" => std::env::var("GU_HUB_CONF_DIR").ok().map(PathBuf::from),
            _ => None,
        };
        app.arg(
            Arg::with_name("config-dir")
                .short("c")
                .takes_value(true)
                .value_name("PATH")
                .help("Set configuration directory path."),
        )
        .arg(
            Arg::with_name("user")
                .long("user")
                .short("u")
                .global(true)
                .help("Set application directories in local user directory (e.g. ~/.local/)"),
        )
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        if matches.is_present("user") {
            set_paths_local()
        }
        /* override config dir path if env variable is defined */
        match *CONFIG_DIR_ENV_VAR_LOCK.read().unwrap() {
            Some(ref path) => set_config_path(path.clone()),
            None => (),
        }
        /* override config dir path if -c argument was used */
        match matches.value_of("config-dir") {
            Some(path) => {
                info!("Using config dir: {}", path);
                set_config_path(PathBuf::from(path));
            }
            _ => (),
        }
        let _ = create_app_dirs().map_err(|_| error!("Cannot create app dirs. Please use --user option to use local user home dirs (recommended) or -c to set config dir."));
        false
    }
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use super::{ConfigSection, HasSectionId};

    #[derive(Deserialize, Serialize, Default)]
    struct Test {
        a: String,
        b: u64,
    }

    impl HasSectionId for Test {
        const SECTION_ID: &'static str = "S_TEST";
    }

    #[test]
    fn test_json() {
        let t = Test {
            a: "ala".into(),
            b: 12,
        };

        let _b = t.to_json().unwrap();
    }

}
