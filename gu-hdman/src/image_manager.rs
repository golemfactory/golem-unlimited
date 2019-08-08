use std::path::PathBuf;

use actix::prelude::*;
use failure::Fail;
use futures::prelude::*;
use futures::sync::oneshot::Canceled;

use gu_model::envman::Image;
use gu_model::hash::{Error as HashParseError, ParsedHash};

use super::cache::{resolve, CacheProvider};
use super::download::DownloadOptionsBuilder;

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "{}", _0)]
    Other(String),
}

impl From<MailboxError> for Error {
    fn from(e: MailboxError) -> Self {
        Error::Other(format!("{}", e))
    }
}

impl From<Canceled> for Error {
    fn from(e: Canceled) -> Self {
        Error::Other(format!("{}", e))
    }
}

impl From<HashParseError> for Error {
    fn from(e: HashParseError) -> Self {
        Error::Other(format!("{}", e))
    }
}

#[derive(Clone, Default)]
struct ImageCacheProvider;

impl ImageCacheProvider {
    fn path(&self, hash: &str) -> Result<PathBuf, Error> {
        let h = ParsedHash::from_hash_bytes(hash.as_bytes())?;
        Ok(std::env::temp_dir().join(&h.to_path()?))
    }
}

impl CacheProvider for ImageCacheProvider {
    type Key = String;
    type Hint = Image;
    type Value = PathBuf;
    type Error = Error;
    type CheckResult = Result<Option<PathBuf>, Error>;
    type FetchResult = Box<dyn Future<Item = PathBuf, Error = Error>>;

    fn try_get(&self, key: &Self::Key) -> Self::CheckResult {
        let p = self.path(key)?;

        if p.exists() {
            Ok(Some(p))
        } else {
            Ok(None)
        }
    }

    fn fetch(&mut self, hash: Self::Key, image: Self::Hint) -> Self::FetchResult {
        let p = self.path(&hash).unwrap();

        Box::new(
            DownloadOptionsBuilder::default()
                .download(&image.url, p.to_string_lossy().into())
                .for_each(|progress| Ok(eprintln!("progress={:?}", progress)))
                .and_then(|_v| Ok(p))
                .map_err(|e| Error::Other(format!("{}", e))),
        )
    }
}

pub fn image(spec: Image) -> impl Future<Item = PathBuf, Error = Error> {
    resolve::<ImageCacheProvider>(spec.hash.clone(), spec)
}
