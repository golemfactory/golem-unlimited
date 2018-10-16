#![allow(proc_macro_derive_resolution_fallback)]

use super::session::Session;
use actix::Handler;
use actix::MessageResult;
use actix::Supervised;
use actix::SystemService;
use actix::{Actor, Context};
use gu_persist::config::ConfigModule;
use serde_json::Value;
use sessions::blob::Blob;
use sessions::responses::SessionErr;
use sessions::responses::SessionOk;
use sessions::responses::SessionResult;
use sessions::session::SessionInfo;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct SessionsManager {
    path: PathBuf,
    next_id: u64,
    sessions: HashMap<u64, Session>,
}

impl Default for SessionsManager {
    fn default() -> Self {
        let path = ConfigModule::new().work_dir().join("sessions");
        // TODO:
        let _ = fs::DirBuilder::new().create(&path);

        SessionsManager {
            path,
            next_id: 0,
            sessions: HashMap::new(),
        }
    }
}

impl SessionsManager {
    pub fn list_sessions(&self) -> SessionResult {
        Ok(SessionOk::SessionsList(
            self.sessions.values().map(|s| s.info()).collect(),
        ))
    }

    pub fn create_session(&mut self, info: SessionInfo) -> SessionResult {
        let id = self.next_id;
        self.next_id += 1;

        match self
            .sessions
            .insert(id, Session::new(info, self.path.join(format!("{}", id))))
        {
            Some(_) => Err(SessionErr::OverwriteError),
            None => Ok(SessionOk::SessionId(id)),
        }
    }

    pub fn session_info(&self, id: u64) -> SessionResult {
        match self.sessions.get(&id) {
            Some(s) => Ok(SessionOk::SessionInfo(s.info())),
            None => Err(SessionErr::SessionNotFoundError),
        }
    }

    pub fn delete_session(&mut self, id: u64) -> SessionResult {
        match self.sessions.remove(&id) {
            Some(_) => Ok(SessionOk::Ok),
            None => Ok(SessionOk::SessionAlreadyDeleted),
        }
    }

    pub fn get_config(&self, id: u64) -> SessionResult {
        match self.sessions.get(&id) {
            Some(s) => s.metadata(),
            None => Err(SessionErr::SessionNotFoundError),
        }
    }

    pub fn set_config(&mut self, id: u64, val: Value) -> SessionResult {
        match self.sessions.get_mut(&id) {
            Some(s) => s.set_metadata(val),
            None => Err(SessionErr::SessionNotFoundError),
        }
    }

    pub fn create_blob(&mut self, id: u64) -> SessionResult {
        match self.sessions.get_mut(&id) {
            Some(s) => s.new_blob(),
            None => Err(SessionErr::SessionNotFoundError),
        }
    }

    pub fn set_blob(&mut self, id: u64, b_id: u64, blob: Blob) -> SessionResult {
        match self.sessions.get_mut(&id) {
            Some(s) => s.set_blob(b_id, blob),
            None => Err(SessionErr::SessionNotFoundError),
        }
    }

    pub fn get_blob(&self, id: u64, b_id: u64) -> SessionResult {
        match self.sessions.get(&id) {
            Some(s) => s.get_blob(b_id),
            None => Err(SessionErr::SessionNotFoundError),
        }
    }

    pub fn delete_blob(&mut self, id: u64, b_id: u64) -> SessionResult {
        match self.sessions.get_mut(&id) {
            Some(s) => s.delete_blob(b_id),
            None => Err(SessionErr::SessionNotFoundError),
        }
    }
}

impl Actor for SessionsManager {
    type Context = Context<Self>;
}

impl Supervised for SessionsManager {}

impl SystemService for SessionsManager {}

#[derive(Message)]
#[rtype(result = "SessionResult")]
pub struct ListSessions;

impl Handler<ListSessions> for SessionsManager {
    type Result = MessageResult<ListSessions>;

    fn handle(
        &mut self,
        _msg: ListSessions,
        _ctx: &mut Context<Self>,
    ) -> MessageResult<ListSessions> {
        MessageResult(self.list_sessions())
    }
}

#[derive(Message, Deserialize, Serialize)]
#[rtype(result = "SessionResult")]
pub struct CreateSession {
    pub info: SessionInfo,
}

impl Handler<CreateSession> for SessionsManager {
    type Result = MessageResult<CreateSession>;

    fn handle(
        &mut self,
        msg: CreateSession,
        _ctx: &mut Context<Self>,
    ) -> MessageResult<CreateSession> {
        MessageResult(self.create_session(msg.info))
    }
}

#[derive(Message)]
#[rtype(result = "SessionResult")]
pub struct GetSessionInfo {
    pub session: u64,
}

impl Handler<GetSessionInfo> for SessionsManager {
    type Result = MessageResult<GetSessionInfo>;

    fn handle(
        &mut self,
        msg: GetSessionInfo,
        _ctx: &mut Context<Self>,
    ) -> MessageResult<GetSessionInfo> {
        MessageResult(self.session_info(msg.session))
    }
}

#[derive(Message)]
#[rtype(result = "SessionResult")]
pub struct DeleteSession {
    pub session: u64,
}

impl Handler<DeleteSession> for SessionsManager {
    type Result = MessageResult<DeleteSession>;

    fn handle(
        &mut self,
        msg: DeleteSession,
        _ctx: &mut Context<Self>,
    ) -> MessageResult<DeleteSession> {
        MessageResult(self.delete_session(msg.session))
    }
}

#[derive(Message)]
#[rtype(result = "SessionResult")]
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
#[rtype(result = "SessionResult")]
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
#[rtype(result = "SessionResult")]
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
#[rtype(result = "SessionResult")]
pub struct SetBlob {
    pub session: u64,
    pub blob_id: u64,
    pub blob: Blob,
}

impl Handler<SetBlob> for SessionsManager {
    type Result = MessageResult<SetBlob>;

    fn handle(&mut self, msg: SetBlob, _ctx: &mut Context<Self>) -> MessageResult<SetBlob> {
        MessageResult(self.set_blob(msg.session, msg.blob_id, msg.blob))
    }
}

#[derive(Message)]
#[rtype(result = "SessionResult")]
pub struct GetBlob {
    pub session: u64,
    pub blob_id: u64,
}

impl Handler<GetBlob> for SessionsManager {
    type Result = MessageResult<GetBlob>;

    fn handle(&mut self, msg: GetBlob, _ctx: &mut Context<Self>) -> MessageResult<GetBlob> {
        MessageResult(self.get_blob(msg.session, msg.blob_id))
    }
}

#[derive(Message)]
#[rtype(result = "SessionResult")]
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
