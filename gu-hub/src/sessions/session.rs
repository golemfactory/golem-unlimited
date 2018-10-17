use serde_json::Value;
use sessions::{
    blob::Blob,
    responses::{SessionErr, SessionOk, SessionResult},
};
use std::{collections::HashMap, fs, io, path::PathBuf};

pub struct Session {
    info: SessionInfo,
    state: Value,
    path: PathBuf,
    next_id: u64,
    storage: HashMap<u64, Blob>,
    version: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SessionInfo {
    name: String,
    environment: String,
}

impl Session {
    pub fn new(info: SessionInfo, path: PathBuf) -> io::Result<Self> {
        fs::DirBuilder::new().create(&path)?;

        Ok(Session {
            info,
            state: Value::Null,
            path,
            next_id: 0,
            storage: HashMap::new(),
            version: 0,
        })
    }

    pub fn info(&self) -> SessionInfo {
        self.info.clone()
    }

    pub fn metadata(&self) -> SessionResult {
        Ok(SessionOk::SessionJson(self.state.clone().into()))
    }

    pub fn set_metadata(&mut self, val: Value) -> SessionResult {
        self.version += 1;
        self.state = val;
        Ok(SessionOk::Ok)
    }

    pub fn new_blob(&mut self) -> SessionResult {
        let id = self.next_id;
        self.next_id += 1;
        self.version += 1;

        match self.storage.insert(
            id,
            Blob::new(self.path.join(format!("{}", id)))
                .map_err(|e| SessionErr::FileError(e.to_string()))?,
        ) {
            Some(_) => Err(SessionErr::OverwriteError),
            None => Ok(SessionOk::BlobId(id)),
        }
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
