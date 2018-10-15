#![allow(proc_macro_derive_resolution_fallback)]

use std::collections::HashMap;
use super::session::Session;
use sessions::responses::SessionResponse;
use serde_json::Value;
use sessions::blob::Blob;
use sessions::session::SessionInfo;
use actix::{Actor, Context};
use actix::Handler;
use actix::MessageResult;
use actix::Supervised;
use actix::SystemService;

#[derive(Default)]
pub struct SessionsManager {
    next_id: u64,
    sessions: HashMap<u64, Session>
}

impl SessionsManager {
    pub fn list_sessions(&self) -> SessionResponse {
        SessionResponse::SessionsList(
            self.sessions
                .values()
                .map(|s| s.info()).collect()
        )
    }

    pub fn create_session(&mut self, info: SessionInfo) -> SessionResponse {
        let id = self.next_id;
        self.next_id += 1;

        match self.sessions.insert(id, Session::new(info)) {
            Some(_) => SessionResponse::OverwriteError,
            None => SessionResponse::SessionId(id)
        }
    }

    pub fn session_info(&self, id: u64) -> SessionResponse {
        match self.sessions.get(&id) {
            Some(s) => SessionResponse::SessionInfo(s.info()),
            None => SessionResponse::SessionNotFoundError,
        }
    }

    pub fn delete_session(&mut self, id: u64) -> SessionResponse {
        match self.sessions.remove(&id) {
            Some(_) => SessionResponse::Ok,
            None => SessionResponse::SessionAlreadyDeleted,
        }
    }

    pub fn get_config(&self, id: u64) -> SessionResponse {
        match self.sessions.get(&id) {
            Some(s) => s.metadata(),
            None => SessionResponse::SessionNotFoundError,
        }
    }

    pub fn set_config(&mut self, id: u64, val: Value) -> SessionResponse {
        match self.sessions.get_mut(&id) {
            Some(s) => s.set_metadata(val),
            None => SessionResponse::SessionNotFoundError,
        }
    }

    pub fn create_blob(&mut self, id: u64) -> SessionResponse {
        match self.sessions.get_mut(&id) {
            Some(s) => s.new_blob(),
            None => SessionResponse::SessionNotFoundError,
        }
    }

    pub fn upload_blob(&mut self, id: u64, b_id: u64, blob: Blob) -> SessionResponse {
        match self.sessions.get_mut(&id) {
            Some(s) => s.upload_blob(b_id, blob),
            None => SessionResponse::SessionNotFoundError,
        }
    }

    pub fn download_blob(&self, id: u64, b_id: u64) -> SessionResponse {
        match self.sessions.get(&id) {
            Some(s) => s.download_blob(b_id),
            None => SessionResponse::SessionNotFoundError,
        }
    }

    pub fn delete_blob(&mut self, id: u64, b_id: u64) -> SessionResponse {
        match self.sessions.get_mut(&id) {
            Some(s) => s.delete_blob(b_id),
            None => SessionResponse::SessionNotFoundError,
        }
    }
}

impl Actor for SessionsManager {
    type Context = Context<Self>;
}

impl Supervised for SessionsManager {}

impl SystemService for SessionsManager {}

#[derive(Message)]
#[rtype(result="SessionResponse")]
pub struct ListSessions;

impl Handler<ListSessions> for SessionsManager {
    type Result = MessageResult<ListSessions>;

    fn handle(&mut self, _msg: ListSessions, _ctx: &mut Context<Self>) -> MessageResult<ListSessions> {
        MessageResult(self.list_sessions())
    }
}

#[derive(Message, Deserialize, Serialize)]
#[rtype(result="SessionResponse")]
pub struct CreateSession {
    pub info: SessionInfo
}

impl Handler<CreateSession> for SessionsManager {
    type Result = MessageResult<CreateSession>;

    fn handle(&mut self, msg: CreateSession, _ctx: &mut Context<Self>) -> MessageResult<CreateSession> {
        MessageResult(self.create_session(msg.info))
    }
}

#[derive(Message)]
#[rtype(result="SessionResponse")]
pub struct GetSessionInfo {
    pub session: u64,
}

impl Handler<GetSessionInfo> for SessionsManager {
    type Result = MessageResult<GetSessionInfo>;

    fn handle(&mut self, msg: GetSessionInfo, _ctx: &mut Context<Self>) -> MessageResult<GetSessionInfo> {
        MessageResult(self.session_info(msg.session))
    }
}

#[derive(Message)]
#[rtype(result="SessionResponse")]
pub struct DeleteSession {
    pub session: u64,
}

impl Handler<DeleteSession> for SessionsManager {
    type Result = MessageResult<DeleteSession>;

    fn handle(&mut self, msg: DeleteSession, _ctx: &mut Context<Self>) -> MessageResult<DeleteSession> {
        MessageResult(self.delete_session(msg.session))
    }
}

#[derive(Message)]
#[rtype(result="SessionResponse")]
pub struct GetMetadata {
    pub session: u64,
}

impl Handler<GetMetadata> for SessionsManager {
    type Result = MessageResult<GetMetadata>;

    fn handle(&mut self, msg: GetMetadata, _ctx: &mut Context<Self>) -> MessageResult<GetMetadata> {
        MessageResult(self.get_config(msg.session))
    }
}

#[derive(Message)]
#[rtype(result="SessionResponse")]
pub struct SetMetadata {
    pub session: u64,
    pub metadata: Value,
}

impl Handler<SetMetadata> for SessionsManager {
    type Result = MessageResult<SetMetadata>;

    fn handle(&mut self, msg: SetMetadata, _ctx: &mut Context<Self>) -> MessageResult<SetMetadata> {
        MessageResult(self.set_config(msg.session, msg.metadata))
    }
}

#[derive(Message)]
#[rtype(result="SessionResponse")]
pub struct CreateBlob {
    pub session: u64,
}

impl Handler<CreateBlob> for SessionsManager {
    type Result = MessageResult<CreateBlob>;

    fn handle(&mut self, msg: CreateBlob, _ctx: &mut Context<Self>) -> MessageResult<CreateBlob> {
        MessageResult(self.create_blob(msg.session))
    }
}

#[derive(Message)]
#[rtype(result="SessionResponse")]
pub struct UploadBlob {
    pub session: u64,
    pub blob_id: u64,
    pub blob: Blob
}

impl Handler<UploadBlob> for SessionsManager {
    type Result = MessageResult<UploadBlob>;

    fn handle(&mut self, msg: UploadBlob, _ctx: &mut Context<Self>) -> MessageResult<UploadBlob> {
        MessageResult(self.upload_blob(msg.session, msg.blob_id, msg.blob))
    }
}

#[derive(Message)]
#[rtype(result="SessionResponse")]
pub struct DownloadBlob {
    pub session: u64,
    pub blob_id: u64,
}

impl Handler<DownloadBlob> for SessionsManager {
    type Result = MessageResult<DownloadBlob>;

    fn handle(&mut self, msg: DownloadBlob, _ctx: &mut Context<Self>) -> MessageResult<DownloadBlob> {
        MessageResult(self.download_blob(msg.session, msg.blob_id))
    }
}

#[derive(Message)]
#[rtype(result="SessionResponse")]
pub struct DeleteBlob {
    pub session: u64,
    pub blob_id: u64,
}

impl Handler<DeleteBlob> for SessionsManager {
    type Result = MessageResult<DeleteBlob>;

    fn handle(&mut self, msg: DeleteBlob, _ctx: &mut Context<Self>) -> MessageResult<DeleteBlob> {
        MessageResult(self.delete_blob(msg.session, msg.blob_id))
    }
}