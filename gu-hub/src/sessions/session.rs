use serde_json::Value;
use std::collections::HashMap;
use sessions::blob::Blob;
use sessions::responses::SessionResponse;

pub struct Session {
    info: SessionInfo,
    state: Value,
    next_id: u64,
    storage: HashMap<u64, Blob>
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    name: String,
    environment: String,
}

impl Session
{
    pub fn new(info: SessionInfo) -> Self {
        Session {
            info,
            state: Value::Null,
            next_id: 0,
            storage: HashMap::new(),
        }
    }

    pub fn info(&self) -> SessionInfo {
        self.info.clone()
    }

    pub fn metadata(&self) -> SessionResponse {
        SessionResponse::SessionJson(self.state.clone().into())
    }

    pub fn set_metadata(&mut self, val: Value) -> SessionResponse {
        self.state = val;
        SessionResponse::Ok
    }

    pub fn new_blob(&mut self) -> SessionResponse {
        let id = self.next_id;
        self.next_id += 1;

        match self.storage.insert(id, Blob::new()) {
            Some(_) => SessionResponse::OverwriteError,
            None => SessionResponse::BlobId(id)
        }
    }

    pub fn upload_blob(&mut self, id: u64, blob: Blob) -> SessionResponse {
        match self.storage.insert(id, blob) {
            Some(_) => SessionResponse::Ok,
            None => SessionResponse::Ok,
        }
    }

    pub fn download_blob(&self, id: u64) -> SessionResponse {
        match self.storage.get(&id) {
            Some(b) => SessionResponse::Blob(b.clone()),
            None => SessionResponse::BlobNotFoundError,
        }
    }

    pub fn delete_blob(&mut self, id: u64) -> SessionResponse {
        match self.storage.remove(&id) {
            Some(_) => SessionResponse::Ok,
            None => SessionResponse::BlobAlreadyDeleted,
        }
    }
}