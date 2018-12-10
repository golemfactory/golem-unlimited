//! Session manager.
//!
//! Manages hub session state.
//!
use actix::prelude::*;
use futures::prelude::*;
use gu_actix::prelude::*;
use std::marker::PhantomData;

#[derive(Default)]
pub struct SessionsManager {
    version: u64,
    path: PathBuf,
    next_id: u64,
    sessions: HashMap<u64, Session>,
}

impl Actor for SessionsManager {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        let path = ConfigModule::new().work_dir().join("sessions");

        fs::DirBuilder::new()
            .recursive(true)
            .create(&path)
            .expect("Cannot create sessions directory");

        self.path = path;

        entries_id_iter(&self.path).for_each(|id| {
            match Session::from_existing(self.path.join(format!("{}", id))).wait() {
                Err(e) => error!("{}", e),
                Ok(s) => {
                    let _ = self
                        .create_session_inner(s, Some(id))
                        .map_err(|e| error!("Session creation info: {:?}", e));
                }
            }
        });
    }
}

#[derive(Message)]
#[rtype(result = "Result<u64, SessionErr>")]
/// Creates new hub session
pub struct Create {
    inner: SessionInfo,
}

pub struct Update<F> {
    session_id: u64,
    command: F,
}

impl<Fact, Fut, R> Update<Fact>
where
    Fact: FnOnce(&mut Session) -> Fut,
    Fut: IntoFuture<Item = R, Error = SessionErr> + 'static,
    R: Send + 'static,
{
    pub fn new(session_id: u64, command: Fact) -> Self {
        Update {
            session_id,
            command,
        }
    }
}

impl<Fact, Fut, R> Message for Update<Fact>
where
    Fact: FnOnce(&mut Session) -> Fut,
    Fut: IntoFuture<Item = R, Error = SessionErr>,
    R: Send + 'static,
{
    type Result = Result<R, SessionErr>;
}

#[derive(Message)]
#[rtype(result = "Result<(), SessionErr>")]
pub struct Delete {
    session_id: u64,
}

impl<Fact, Fut, R> Handler<Update<Fact>> for SessionsManager
where
    Fact: FnOnce(&mut Session) -> Fut,
    Fut: IntoFuture<Item = R, Error = SessionErr> + 'static,
    R: Send + 'static,
{
    type Result = ActorResponse<SessionsManager, R, SessionErr>;

    fn handle(&mut self, msg: Update<Fact>, ctx: &mut Self::Context) -> Self::Result {
        if let Some(session) = self.sessions.get_mut(&msg.session_id) {
            let command = msg.command;
            let result = command(session).into_future();

            ActorResponse::async(actix::fut::wrap_future(result))
        } else {
            ActorResponse::reply(Err(SessionErr::SessionNotFoundError))
        }
    }
}

use super::session::Session;
use actix::{
    Actor, ActorResponse, Context, Handler, MessageResult, Supervised, SystemService, WrapFuture,
};
use futures::{future, Future, IntoFuture};
use gu_model::session::Metadata;
use gu_persist::config::ConfigModule;
use serde_json::Value;
use sessions::{
    blob::Blob,
    responses::{SessionErr, SessionOk, SessionResult},
    session::{entries_id_iter, SessionInfo},
};
use std::{cmp, collections::HashMap, fs, path::PathBuf};

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

    fn session_fn<R, F>(&self, id: u64, f: F) -> Result<R, SessionErr>
    where
        F: FnOnce(&Session) -> Result<R, SessionErr>,
    {
        match self.sessions.get(&id) {
            Some(s) => f(s),
            None => Err(SessionErr::SessionNotFoundError),
        }
    }

    fn session_mut_fn<R, F>(&mut self, id: u64, f: F) -> Result<R, SessionErr>
    where
        F: FnOnce(&mut Session) -> Result<R, SessionErr>,
    {
        self.version += 1;
        match self.sessions.get_mut(&id) {
            Some(s) => f(s),
            None => Err(SessionErr::SessionNotFoundError),
        }
    }

    fn create_session_inner(
        &mut self,
        session: Session,
        id: Option<u64>,
    ) -> Result<u64, SessionErr> {
        let id = match id {
            None => self.next_id,
            Some(v) => v,
        };
        self.next_id = cmp::max(id, self.next_id) + 1;
        self.version += 1;

        match self.sessions.insert(id, session) {
            Some(_) => Err(SessionErr::OverwriteError),
            None => Ok(id),
        }
    }

    pub fn create_session(
        &mut self,
        info: SessionInfo,
    ) -> impl Future<Item = u64, Error = SessionErr> {
        let (session, fut) = Session::new(info, self.path.join(format!("{}", self.next_id)));

        self.create_session_inner(session, None).into_future()
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

    pub fn get_config(&self, id: u64) -> Result<Metadata, SessionErr> {
        self.session_fn(id, |s| Ok(s.metadata().clone()))
    }

    pub fn set_config(
        &mut self,
        id: u64,
        val: Metadata,
    ) -> impl Future<Item = u64, Error = SessionErr> {
        self.version += 1;
        match self.sessions.get_mut(&id) {
            None => future::Either::A(future::err(SessionErr::SessionNotFoundError)),
            Some(s) => future::Either::B(s.set_metadata(val)),
        }
    }

    pub fn create_blob(&mut self, id: u64) -> Result<(u64, Blob), SessionErr> {
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
#[rtype(result = "Result<u64, SessionErr>")]
pub struct CreateSession {
    pub info: SessionInfo,
}

impl Handler<CreateSession> for SessionsManager {
    type Result = ActorResponse<SessionsManager, u64, SessionErr>;

    fn handle(&mut self, msg: CreateSession, _ctx: &mut Context<Self>) -> Self::Result {
        ActorResponse::async(self.create_session(msg.info).into_actor(self))
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
#[rtype(result = "Result<Metadata, SessionErr>")]
pub struct GetMetadata {
    pub session: u64,
}

impl Handler<GetMetadata> for SessionsManager {
    type Result = Result<Metadata, SessionErr>;

    fn handle(&mut self, msg: GetMetadata, _ctx: &mut Context<Self>) -> Self::Result {
        self.get_config(msg.session)
    }
}

#[derive(Message)]
#[rtype(result = "Result<u64, SessionErr>")]
pub struct SetMetadata {
    pub session: u64,
    pub metadata: Metadata,
}

impl Handler<SetMetadata> for SessionsManager {
    type Result = ActorResponse<SessionsManager, u64, SessionErr>;

    fn handle(&mut self, msg: SetMetadata, _ctx: &mut Context<Self>) -> Self::Result {
        ActorResponse::async(self.set_config(msg.session, msg.metadata).into_actor(self))
    }
}

#[derive(Message)]
#[rtype(result = "Result<(u64, Blob), SessionErr>")]
pub struct CreateBlob {
    pub session: u64,
}

impl Handler<CreateBlob> for SessionsManager {
    type Result = Result<(u64, Blob), SessionErr>;

    fn handle(&mut self, msg: CreateBlob, _ctx: &mut Context<Self>) -> Self::Result {
        self.create_blob(msg.session)
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
