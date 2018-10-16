use serde_json::Value;
use sessions::blob::Blob;
use sessions::responses::SessionErr;
use sessions::responses::SessionOk;
use sessions::responses::SessionResult;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct Session {
    info: SessionInfo,
    state: Value,
    path: PathBuf,
    next_id: u64,
    storage: HashMap<u64, Blob>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SessionInfo {
    name: String,
    environment: String,
}

impl Session {
    pub fn new(info: SessionInfo, path: PathBuf) -> Self {
        // TODO:
        let _ = fs::DirBuilder::new().create(&path);

        Session {
            info,
            state: Value::Null,
            path,
            next_id: 0,
            storage: HashMap::new(),
        }
    }

    pub fn info(&self) -> SessionInfo {
        self.info.clone()
    }

    pub fn metadata(&self) -> SessionResult {
        Ok(SessionOk::SessionJson(self.state.clone().into()))
    }

    pub fn set_metadata(&mut self, val: Value) -> SessionResult {
        self.state = val;
        Ok(SessionOk::Ok)
    }

    pub fn new_blob(&mut self) -> SessionResult {
        let id = self.next_id;
        self.next_id += 1;

        match self
            .storage
            .insert(id, Blob::new(self.path.join(format!("{}", id))))
        {
            Some(_) => Err(SessionErr::OverwriteError),
            None => Ok(SessionOk::BlobId(id)),
        }
    }

    pub fn set_blob(&mut self, id: u64, blob: Blob) -> SessionResult {
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
        match self.storage.remove(&id) {
            Some(_) => Ok(SessionOk::Ok),
            None => Ok(SessionOk::BlobAlreadyDeleted),
        }
    }
}
