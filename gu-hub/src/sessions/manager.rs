#![allow(proc_macro_derive_resolution_fallback)]

use super::session::Session;
use actix::{
    Actor, ActorResponse, Context, Handler, MessageResult, Supervised, SystemService, WrapFuture,
};
use futures::{future, Future, IntoFuture};
use gu_persist::config::ConfigModule;
use serde_json::Value;
use sessions::{
    blob::Blob,
    responses::{SessionErr, SessionOk, SessionResult},
    session::{entries_id_iter, SessionInfo},
};
use std::{cmp, collections::HashMap, fs, path::PathBuf};

pub struct SessionsManager {
    path: PathBuf,
    next_id: u64,
    sessions: HashMap<u64, Session>,
    version: u64,
}

impl Default for SessionsManager {
    fn default() -> Self {
        let path = ConfigModule::new().work_dir().join("sessions");
        fs::DirBuilder::new()
            .recursive(true)
            .create(&path)
            .expect("Cannot create sessions directory");

        let mut m = SessionsManager {
            path: path.clone(),
            next_id: 0,
            sessions: HashMap::new(),
            version: 0,
        };

        entries_id_iter(&path).for_each(|id| {
            match Session::from_existing(path.join(format!("{}", id))).wait() {
                Err(e) => error!("{}", e),
                Ok(s) => {
                    let _ = m
                        .create_session_inner(s, Some(id))
                        .map_err(|e| error!("Session creation info: {:?}", e));
                }
            }
        });

        m
    }
}

impl SessionsManager {
    pub fn list_sessions(&self) -> SessionResult {
        Ok(SessionOk::SessionsList(
            self.sessions
                .iter()
                .map(|(id, s)| EnumeratedSessionInfo {
                    id: *id,
                    info: s.info(),
                })
                .collect(),
            self.version,
        ))
    }

    fn session_fn<F: FnOnce(&Session) -> SessionResult>(&self, id: u64, f: F) -> SessionResult {
        match self.sessions.get(&id) {
            Some(s) => f(s),
            None => Err(SessionErr::SessionNotFoundError),
        }
    }

    fn session_mut_fn<F: FnOnce(&mut Session) -> SessionResult>(
        &mut self,
        id: u64,
        f: F,
    ) -> SessionResult {
        self.version += 1;
        match self.sessions.get_mut(&id) {
            Some(s) => f(s),
            None => Err(SessionErr::SessionNotFoundError),
        }
    }

    fn create_session_inner(&mut self, session: Session, id: Option<u64>) -> SessionResult {
        let id = match id {
            None => self.next_id,
            Some(v) => v,
        };
        self.next_id = cmp::max(id, self.next_id) + 1;
        self.version += 1;

        match self.sessions.insert(id, session) {
            Some(_) => Err(SessionErr::OverwriteError),
            None => Ok(SessionOk::SessionId(id)),
        }
    }

    pub fn create_session(
        &mut self,
        info: SessionInfo,
    ) -> impl Future<Item = SessionOk, Error = SessionErr> {
        let (session, fut) = Session::new(info, self.path.join(format!("{}", self.next_id)));

        self.create_session_inner(session, None)
            .into_future()
            .and_then(|res| fut.and_then(|()| Ok(res)))
    }

    pub fn session_info(&self, id: u64) -> SessionResult {
        self.session_fn(id, |s| Ok(SessionOk::SessionInfo(s.info(), self.version)))
    }

    pub fn delete_session(&mut self, id: u64) -> SessionResult {
        self.version += 1;
        match self.sessions.remove(&id).map(|mut s| s.clean_directory()) {
            Some(Ok(())) => Ok(SessionOk::Ok),
            Some(Err(e)) => Err(SessionErr::FileError(e.to_string())),
            None => Ok(SessionOk::SessionAlreadyDeleted),
        }
    }

    pub fn get_config(&self, id: u64) -> SessionResult {
        self.session_fn(id, |s| s.metadata())
    }

    pub fn set_config(
        &mut self,
        id: u64,
        val: Value,
    ) -> impl Future<Item = SessionOk, Error = SessionErr> {
        self.version += 1;
        match self.sessions.get_mut(&id) {
            None => future::Either::A(future::err(SessionErr::SessionNotFoundError)),
            Some(s) => future::Either::B(s.set_metadata(val)),
        }
    }

    pub fn create_blob(&mut self, id: u64) -> SessionResult {
        self.session_mut_fn(id, |s| s.new_blob())
    }

    pub fn set_blob(&mut self, id: u64, b_id: u64, blob: Blob) -> SessionResult {
        self.session_mut_fn(id, |s| s.set_blob(b_id, blob))
    }

    pub fn get_blob(&self, id: u64, b_id: u64) -> SessionResult {
        self.session_fn(id, |s| s.get_blob(b_id))
    }

    pub fn delete_blob(&mut self, id: u64, b_id: u64) -> SessionResult {
        self.session_mut_fn(id, |s| s.delete_blob(b_id))
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct EnumeratedSessionInfo {
    id: u64,
    #[serde(flatten)]
    info: SessionInfo,
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
    type Result = ActorResponse<SessionsManager, SessionOk, SessionErr>;

    fn handle(&mut self, msg: CreateSession, _ctx: &mut Context<Self>) -> Self::Result {
        ActorResponse::async(self.create_session(msg.info).into_actor(self))
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
    type Result = ActorResponse<SessionsManager, SessionOk, SessionErr>;

    fn handle(&mut self, msg: SetMetadata, _ctx: &mut Context<Self>) -> Self::Result {
        ActorResponse::async(self.set_config(msg.session, msg.metadata).into_actor(self))
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
