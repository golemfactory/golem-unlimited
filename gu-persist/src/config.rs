#![allow(proc_macro_derive_resolution_fallback)]

use std::{
    any::Any,
    borrow::Cow,
    collections::HashMap,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Arc,
};

use actix::{fut, prelude::*};
use directories::ProjectDirs;
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value as JsonValue};

use gu_actix::*;
use gu_base::{App, Arg, Module};

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
        let storage = match self.storage.take() {
            Some(v) => v,
            None => SyncArbiter::start(1, || Storage::from_path(default_config_dir())),
        };
        self.storage = Some(storage);
        self.storage.as_ref().unwrap()
    }
}

impl Actor for ConfigManager {
    type Context = Context<Self>;
}

fn default_config_dir() -> PathBuf {
    use directories::ProjectDirs;

    let p = ProjectDirs::from("network", "Golem", "Golem Unlimited").unwrap();

    p.config_dir().into()
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

#[derive(Message)]
#[rtype(result = "Result<()>")]
pub enum SetConfigPath {
    Default(Cow<'static, str>),
    FsPath(Cow<'static, str>),
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

impl Handler<SetConfigPath> for ConfigManager {
    type Result = Result<()>;

    fn handle(&mut self, msg: SetConfigPath, _ctx: &mut Self::Context) -> Self::Result {
        let path = match msg {
            SetConfigPath::Default(app_name) => {
                use directories::ProjectDirs;

                ProjectDirs::from("network", "Golem", app_name.as_ref())
                    .unwrap()
                    .config_dir()
                    .into()
            }
            SetConfigPath::FsPath(path) => PathBuf::from(path.as_ref()),
        };

        info!("new config path={:?}", path);

        let file_storage = SyncArbiter::start(1, move || Storage::from_path(&path));

        self.storage = Some(file_storage);

        Ok(())
    }
}

pub struct ConfigModule(ProjectDirs);

impl ConfigModule {
    const KEYSTORE_FILE: &'static str = "keystore.json";

    pub fn new() -> Self {
        ConfigModule(ProjectDirs::from("network", "Golem", "Golem Unlimited").unwrap())
    }

    /// TODO: for extracted sessions
    pub fn work_dir(&self) -> &Path {
        self.0.data_local_dir()
    }

    /// TODO: for downloaded images
    pub fn cache_dir(&self) -> &Path {
        self.0.cache_dir()
    }

    /// TODO: for configs and ethkeys
    pub fn config_dir(&self) -> &Path {
        self.0.config_dir()
    }

    pub fn keystore_path(&self) -> PathBuf {
        self.config_dir()
            .to_path_buf()
            .join(ConfigModule::KEYSTORE_FILE)
    }
}

impl Module for ConfigModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.arg(
            Arg::with_name("config-dir")
                .short("c")
                .takes_value(true)
                .value_name("PATH")
                .help("config dir path"),
        )
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
