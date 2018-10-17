use bytes::Bytes;
use futures::future::IntoFuture;
use futures::stream;
use futures::Future;
use gu_base::files::write_async;
use serde_json;
use serde_json::Value;
use sessions::{
    blob::Blob,
    responses::{SessionErr, SessionOk, SessionResult},
};
use std::cmp;
use std::{collections::HashMap, fs, io, path::PathBuf};

pub struct Session {
    info: SessionInfo,
    state: Value,
    path: PathBuf,
    next_id: u64,
    storage: HashMap<u64, Blob>,
    version: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct SessionInfo {
    name: String,
    environment: String,
}

pub(crate) fn entries_id_iter(path: &PathBuf) -> impl Iterator<Item = u64> {
    fs::read_dir(path)
        .expect("Cannot read session directory")
        .into_iter()
        .map(|entry| {
            entry
                .map_err(|err| error!("Invalid session file: {}", err))
                .and_then(|e| {
                    e.file_name()
                        .to_str()
                        .ok_or_else(|| error!("Invalid session filename"))
                        .and_then(|s| {
                            s.clone().parse::<u64>().map_err(|e| {
                                if !s.starts_with('.') {
                                    error!("Invalid session filename: {}", e)
                                }
                            })
                        })
                })
        }).filter(|res| res.is_ok())
        .map(|id| id.unwrap())
}

impl Session {
    pub fn new(
        info: SessionInfo,
        path: PathBuf,
    ) -> (Session, impl Future<Item = (), Error = SessionErr>) {
        let info_bytes = serde_json::to_string(&info)
            .map_err(|_| SessionErr::FileError("Invalid info file".to_string()))
            .and_then(|s| Ok(Bytes::from(s)));

        let session = Session {
            info,
            state: Value::Null,
            path: path.clone(),
            next_id: 0,
            storage: HashMap::new(),
            version: 0,
        };

        let fut = fs::DirBuilder::new()
            .create(&path)
            .map_err(|e| SessionErr::DirectoryCreationError(e.to_string()))
            .into_future()
            .and_then(move |_| info_bytes)
            .and_then(move |info| {
                write_async(stream::once::<_, ()>(Ok(info)), path.join(".info"))
                    .map_err(|e| SessionErr::FileError(e))
            });

        (session, fut)
    }

    pub fn from_existing(path: PathBuf) -> Self {
        let mut s = Session {
            info: SessionInfo::default(),
            state: Value::Null,
            path: path.clone(),
            next_id: 0,
            storage: HashMap::new(),
            version: 0,
        };

        entries_id_iter(&path).for_each(|id| {
            let _ = s
                .new_blob_inner(Blob::from_existing(path.join(format!("{}", id))), Some(id))
                .map_err(|e| error!("{:?}", e));
        });

        s
    }

    pub fn info(&self) -> SessionInfo {
        self.info.clone()
    }

    pub fn metadata(&self) -> SessionResult {
        Ok(SessionOk::SessionJson(self.state.clone().into()))
    }

    pub fn set_metadata(
        &mut self,
        val: Value,
    ) -> impl Future<Item = SessionOk, Error = SessionErr> {
        self.version += 1;
        self.state = val.clone();

        write_async(
            stream::once::<_, ()>(Ok(Bytes::from(val.to_string()))),
            self.path.join(".json"),
        ).map_err(|e| SessionErr::FileError(e))
        .and_then(|_| Ok(SessionOk::Ok))
    }

    fn new_blob_inner(&mut self, blob: Blob, id: Option<u64>) -> SessionResult {
        let id = match id {
            None => self.next_id,
            Some(v) => v,
        };
        self.next_id = cmp::max(id, self.next_id) + 1;
        self.version += 1;

        match self.storage.insert(id, blob) {
            Some(_) => Err(SessionErr::OverwriteError),
            None => Ok(SessionOk::BlobId(id)),
        }
    }

    pub fn new_blob(&mut self) -> SessionResult {
        let blob = Blob::new(self.path.join(format!("{}", self.next_id)))
            .map_err(|e| SessionErr::FileError(e.to_string()))?;
        self.new_blob_inner(blob, None)
    }

    pub fn set_blob(&mut self, id: u64, blob: Blob) -> SessionResult {
        self.version += 1;
        match self.storage.insert(id, blob) {
            Some(_) => Ok(SessionOk::Ok),
            None => Ok(SessionOk::Ok),
        }
    }

    pub fn get_blob(&self, id: u64) -> SessionResult {
        match self.storage.get(&id) {
            Some(b) => Ok(SessionOk::Blob(b.clone())),
            None => Err(SessionErr::BlobNotFoundError),
        }
    }

    pub fn delete_blob(&mut self, id: u64) -> SessionResult {
        self.version += 1;
        match self.storage.remove(&id).map(|b| b.clean_file()) {
            Some(Ok(())) => Ok(SessionOk::Ok),
            Some(Err(e)) => Err(SessionErr::FileError(e.to_string())),
            None => Ok(SessionOk::BlobAlreadyDeleted),
        }
    }

    pub fn clean_directory(&mut self) -> io::Result<()> {
        self.version += 1;
        match (&self.path).exists() {
            true => fs::remove_dir_all(&self.path),
            false => Ok(()),
        }
    }
}
