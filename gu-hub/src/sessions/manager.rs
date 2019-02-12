//! Session manager.
//!
//! Manages hub session state.
//!
use actix::prelude::*;
use futures::prelude::*;
use gu_actix::prelude::*;
use gu_net::NodeId;
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

    fn started(&mut self, _ctx: &mut <Self as Actor>::Context) {
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

impl Create {
    pub fn from_info(inner: SessionInfo) -> Self {
        Create { inner }
    }
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

impl Delete {
    pub fn with_session_id(session_id: u64) -> Delete {
        Delete { session_id }
    }
}

impl<Fact, Fut, R> Handler<Update<Fact>> for SessionsManager
where
    Fact: FnOnce(&mut Session) -> Fut,
    Fut: IntoFuture<Item = R, Error = SessionErr> + 'static,
    R: Send + 'static,
{
    type Result = ActorResponse<SessionsManager, R, SessionErr>;

    fn handle(&mut self, msg: Update<Fact>, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(session) = self.sessions.get_mut(&msg.session_id) {
            let command = msg.command;
            let result = command(session).into_future();

            ActorResponse::r#async(actix::fut::wrap_future(result))
        } else {
            ActorResponse::reply(Err(SessionErr::SessionNotFoundError))
        }
    }
}

use super::session::Session;
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
        let (session, _fut) = Session::new(info, self.path.join(format!("{}", self.next_id)));

        self.create_session_inner(session, None).into_future()
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
#[rtype(result = "Result<Vec<(u64, SessionInfo)>, SessionErr>")]
pub struct List;

impl Handler<List> for SessionsManager {
    type Result = Result<Vec<(u64, SessionInfo)>, SessionErr>;

    fn handle(&mut self, _msg: List, _ctx: &mut Context<Self>) -> Self::Result {
        Ok(self
            .sessions
            .iter()
            .map(|(session_id, session)| (*session_id, session.info()))
            .collect())
    }
}

impl Handler<Create> for SessionsManager {
    type Result = ActorResponse<SessionsManager, u64, SessionErr>;

    fn handle(&mut self, msg: Create, _ctx: &mut Context<Self>) -> Self::Result {
        ActorResponse::r#async(self.create_session(msg.inner).into_actor(self))
    }
}

impl Handler<Delete> for SessionsManager {
    type Result = ActorResponse<SessionsManager, (), SessionErr>;

    fn handle(&mut self, msg: Delete, _ctx: &mut Context<Self>) -> Self::Result {
        let mut session = match self.sessions.remove(&msg.session_id) {
            None => return ActorResponse::reply(Err(SessionErr::SessionNotFoundError)),
            Some(session) => session,
        };

        // TODO: This should by async
        match session.clean_directory() {
            Ok(_) => (),
            Err(e) => return ActorResponse::reply(Err(SessionErr::FileError(e.to_string()))),
        }
        ActorResponse::r#async(session.drop_deployments().into_actor(self))
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

#[derive(Message)]
#[rtype(result = "Result<String, SessionErr>")]
pub struct CreateDeployment {
    session_id: u64,
    node_id: NodeId,
    deployment_desc: gu_model::envman::CreateSession,
}

impl CreateDeployment {
    pub fn new(
        session_id: u64,
        node_id: NodeId,
        deployment_desc: gu_model::envman::CreateSession,
    ) -> CreateDeployment {
        CreateDeployment {
            session_id: session_id,
            node_id: node_id,
            deployment_desc: deployment_desc,
        }
    }
}

impl Handler<CreateDeployment> for SessionsManager {
    type Result = ActorResponse<SessionsManager, String, SessionErr>;

    fn handle(&mut self, msg: CreateDeployment, _ctx: &mut Self::Context) -> Self::Result {
        let session_id = msg.session_id;
        let node_id = msg.node_id.clone();

        if let Some(session) = self.sessions.get_mut(&msg.session_id) {
            ActorResponse::r#async(
                fut::wrap_future(session.create_deployment(msg.node_id, msg.deployment_desc))
                    .and_then(move |deployment_id, act: &mut SessionsManager, _ctx| {
                        act.sessions
                            .get_mut(&session_id)
                            .unwrap()
                            .add_deployment(node_id, deployment_id.clone());
                        fut::ok(deployment_id)
                    }),
            )
        } else {
            ActorResponse::reply(Err(SessionErr::SessionNotFoundError))
        }
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), SessionErr>")]
pub struct DeleteDeployment {
    session_id: u64,
    node_id: NodeId,
    deployment_id: String,
}

impl DeleteDeployment {
    pub fn new(session_id: u64, node_id: NodeId, deployment_id: String) -> DeleteDeployment {
        DeleteDeployment {
            session_id: session_id,
            node_id: node_id,
            deployment_id: deployment_id,
        }
    }
}

impl Handler<DeleteDeployment> for SessionsManager {
    type Result = ActorResponse<SessionsManager, (), SessionErr>;

    fn handle(&mut self, msg: DeleteDeployment, _ctx: &mut Self::Context) -> Self::Result {
        let session_id = msg.session_id;
        let node_id = msg.node_id.clone();
        let deployment_id = msg.deployment_id.clone();

        if let Some(session) = self.sessions.get_mut(&msg.session_id) {
            ActorResponse::r#async(
                fut::wrap_future(session.delete_deployment(msg.node_id, msg.deployment_id))
                    .and_then(move |_, act: &mut SessionsManager, _ctx| {
                        if !(act
                            .sessions
                            .get_mut(&session_id)
                            .unwrap()
                            .remove_deployment(node_id, deployment_id.clone()))
                        {
                            return fut::err(SessionErr::DeploymentNotFound(deployment_id));
                        }
                        fut::ok(())
                    }),
            )
        } else {
            ActorResponse::reply(Err(SessionErr::SessionNotFoundError))
        }
    }
}

#[derive(Message)]
#[rtype(result = "Result<Vec<String>, SessionErr>")]
pub struct UpdateDeployment {
    session_id: u64,
    node_id: NodeId,
    deployment_id: String,
    commands: Vec<gu_model::envman::Command>,
}

impl UpdateDeployment {
    pub fn new(
        session_id: u64,
        node_id: NodeId,
        deployment_id: String,
        commands: Vec<gu_model::envman::Command>,
    ) -> UpdateDeployment {
        UpdateDeployment {
            session_id: session_id,
            node_id: node_id,
            deployment_id: deployment_id,
            commands: commands,
        }
    }
}

impl Handler<UpdateDeployment> for SessionsManager {
    type Result = ActorResponse<SessionsManager, Vec<String>, SessionErr>;

    fn handle(&mut self, msg: UpdateDeployment, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(session) = self.sessions.get_mut(&msg.session_id) {
            ActorResponse::r#async(fut::wrap_future(session.update_deployment(
                msg.node_id,
                msg.deployment_id,
                msg.commands,
            )))
        } else {
            ActorResponse::reply(Err(SessionErr::SessionNotFoundError))
        }
    }
}
