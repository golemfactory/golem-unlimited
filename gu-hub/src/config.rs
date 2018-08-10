use actix::prelude::*;
use actix::MailboxError;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value as JsonValue};
use std::borrow::Cow;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

error_chain!(

    //
    foreign_links {
        Json(serde_json::Error);
    }

    errors {
        MailboxError(e : MailboxError){}
    }


);

impl From<MailboxError> for Error {
    fn from(e: MailboxError) -> Self {
        ErrorKind::MailboxError(e).into()
    }
}

pub struct ConfigManager {
    config_dir: PathBuf,
}

impl Actor for ConfigManager {
    type Context = Context<Self>;
}

fn default_config_dir() -> PathBuf {
    use directories::ProjectDirs;

    let p = ProjectDirs::from("network", "Golem", "Unlimited HUB").unwrap();

    p.config_dir().into()
}

impl Default for ConfigManager {
    fn default() -> Self {
        ConfigManager {
            config_dir: default_config_dir(),
        }
    }
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

impl<T: ConfigSection + 'static> Handler<GetConfig<T>> for ConfigManager {
    type Result = Result<Arc<T>>;

    fn handle(&mut self, msg: GetConfig<T>, ctx: &mut Self::Context) -> Self::Result {
        Ok(Arc::new(T::default()))
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
