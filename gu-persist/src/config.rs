pub use super::error::*;
use super::storage::{Fetch, Put};
use actix::MailboxError;
use actix::{fut, prelude::*};
use serde::{Deserialize, Serialize};
use serde_json::{self, Value as JsonValue};
use std::borrow::Cow;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::ops::Deref;

use std::collections::HashMap;
use std::any::Any;

type Storage = super::file_storage::FileStorage;


#[derive(Default)]
pub struct ConfigManager {
    storage: Option<Addr<Storage>>,
    cache : HashMap<&'static str, Box<Any + 'static>>
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

    let p = ProjectDirs::from("network", "Golem", "Unlimited").unwrap();

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
    pub fn new() -> GetConfig<T> {
        GetConfig(PhantomData)
    }
}

impl<T: ConfigSection + 'static> Message for GetConfig<T> {
    type Result = Result<Arc<T>>;
}

#[derive(Message)]
#[rtype(result="Result<()>")]
pub struct SetConfig<T : ConfigSection>(Arc<T>);



impl<T: ConfigSection + 'static> Handler<GetConfig<T>> for ConfigManager {
    type Result = ActorResponse<ConfigManager, Arc<T>, Error>;

    fn handle(&mut self, msg: GetConfig<T>, ctx: &mut Self::Context) -> Self::Result {
        let key = T::SECTION_ID;

        if let Some(ref v) = self.cache.get(key) {
            let y = v.downcast_ref::<Arc<T>>();

            if let Some(v) = y {
                return ActorResponse::reply(Ok(v.clone()))
            }

        }

        ActorResponse::async(
            self.storage()
                .send(Fetch(Cow::Borrowed(T::SECTION_ID)))
                .flatten_fut()
                .into_actor(self)
                .and_then(|r, act, ctx| {
                    println!("{:?}", r);
                    match r {
                        None => {
                            let v = Arc::new(T::default());

                            ctx.notify(SetConfig(v.clone()));

                            fut::ok(v)
                        }
                        Some(v) => {
                            let p : JsonValue = serde_json::from_slice(v.as_ref()).unwrap();
                            fut::ok(Arc::new(T::from_json(p).unwrap()))
                        }
                    }
                }),
        )
    }
}

impl<T : ConfigSection> Handler<SetConfig<T>> for ConfigManager {
    type Result = ActorResponse<ConfigManager, (), Error>;

    fn handle(&mut self, msg: SetConfig<T>, ctx: &mut Self::Context) -> Self::Result {
        let k = Cow::Borrowed(T::SECTION_ID);
        //self.storage().send()
        unimplemented!()
    }
}

#[cfg(test)]
mod test {

    use super::{ConfigSection, HasSectionId};
    use std::borrow::Cow;

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

        let b = t.to_json().unwrap();
    }

}
