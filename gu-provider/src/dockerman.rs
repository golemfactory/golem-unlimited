//! Docker mode implementation

use super::envman;
use actix::prelude::*;
use async_docker::models::ContainerConfig;
use async_docker::{self, new_docker, DockerApi};
use futures::prelude::*;
use gu_model::dockerman::CreateOptions;
use gu_model::envman::*;
use gu_net::rpc::peer::PeerSessionInfo;
use std::collections::BTreeMap;
use workspace::Workspace;

// Actor.
#[derive(Default)]
struct DockerMan {
    docker_api: Option<Box<DockerApi>>,
    sessions: BTreeMap<String, DockerSessionInfo>,
}

struct DockerSessionInfo {
    workspace: Workspace,
}

impl Actor for DockerMan {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        match new_docker(None) {
            Ok(docker_api) => {
                self.docker_api = Some(docker_api);
                envman::register("docker", ctx.address())
            }
            Err(e) => {
                error!("docker start failed: {}", e);
                ctx.stop()
            }
        }
    }
}

impl envman::EnvManService for DockerMan {
    type CreateOptions = CreateOptions;
}

impl Handler<CreateSession<CreateOptions>> for DockerMan {
    type Result = ActorResponse<DockerMan, String, Error>;

    fn handle(
        &mut self,
        msg: CreateSession<CreateOptions>,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<CreateSession<CreateOptions>>>::Result {
        debug!("create session for: {}", &msg.image.url);

        match self.docker_api {
            Some(ref api) => {
                let Image { url, hash } = msg.image;

                //let docker_image = api.image(url.as_ref().into());
                let opts = ContainerConfig::new()
                    .with_image(url.into())
                    .with_tty(true)
                    .with_open_stdin(true)
                    .with_attach_stdin(true)
                    .with_attach_stderr(true)
                    .with_attach_stdout(true)
                    .with_volumes(
                        [("/workspace".to_string(), json!({}))]
                            .to_vec()
                            .into_iter()
                            .collect(),
                    )
                    .with_host_config(async_docker::models::HostConfig::new());

                //--.with_volumes([("/gu-data".to_string(), json!({}))].to_vec().into_iter().collect());

                info!("config: {:?}", &opts);

                ActorResponse::async(
                    api.containers()
                        .create(&opts)
                        .and_then(|c| Ok(c.id().to_owned()))
                        .map_err(|e| Error::IoError(format!("{}", e)))
                        .into_actor(self),
                )
            }
            None => ActorResponse::reply(Err(Error::UnknownEnv(msg.env_type))),
        }
    }
}

impl Handler<SessionUpdate> for DockerMan {
    type Result = ActorResponse<DockerMan, Vec<String>, Vec<String>>;

    fn handle(
        &mut self,
        msg: SessionUpdate,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<SessionUpdate>>::Result {
        if !self.sessions.contains_key(&msg.session_id) {
            return ActorResponse::reply(Err(
                vec![Error::NoSuchSession(msg.session_id).to_string()],
            ));
        }

        let mut future_chain: Box<
            ActorFuture<Item = Vec<String>, Error = Vec<String>, Actor = Self>,
        > = Box::new(fut::ok(Vec::new()));

        for cmd in msg.commands {
            let session_id = msg.session_id.clone();

            match cmd {
                Command::Open { args } => (),
                Command::Close => (),
                Command::Exec { executable, args } => (),
                Command::Start { executable, args } => (),
                Command::Stop { child_id } => (),
                Command::AddTags(mut tags) => {
                    future_chain = Box::new(future_chain.and_then(move |mut v, act, _ctx| {
                        match act
                            .sessions
                            .get_mut(&session_id)
                            .ok_or(Error::NoSuchSession(session_id.clone()))
                        {
                            Ok(session) => {
                                tags.into_iter().for_each(|tag| {
                                    session.workspace.add_tags(vec![tag]);
                                });
                                v.push(format!(
                                    "tags inserted. Current tags are: {:?}",
                                    &session.workspace.get_tags()
                                ));
                                fut::ok(v)
                            }
                            Err(e) => {
                                v.push(e.to_string());
                                fut::err(v)
                            }
                        }
                    }));
                }
                Command::DelTags(mut tags) => {
                    future_chain = Box::new(future_chain.and_then(move |mut v, act, _ctx| {
                        match act
                            .sessions
                            .get_mut(&session_id)
                            .ok_or(Error::NoSuchSession(session_id.clone()))
                        {
                            Ok(session) => {
                                session.workspace.remove_tags(tags);
                                v.push(format!(
                                    "tags removed. Current tags are: {:?}",
                                    &session.workspace.get_tags()
                                ));
                                fut::ok(v)
                            }
                            Err(e) => {
                                v.push(e.to_string());
                                fut::err(v)
                            }
                        }
                    }));
                }
                Command::DownloadFile {
                    uri,
                    file_path,
                    format,
                } => (),
                Command::UploadFile {
                    uri,
                    file_path,
                    format,
                } => (),
            }
        }
        ActorResponse::async(future_chain)
    }
}

impl Handler<GetSessions> for DockerMan {
    type Result = ActorResponse<DockerMan, Vec<PeerSessionInfo>, ()>;

    fn handle(
        &mut self,
        _msg: GetSessions,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<GetSessions>>::Result {
        ActorResponse::reply(Ok(Vec::new()))
    }
}

impl Handler<DestroySession> for DockerMan {
    type Result = ActorResponse<DockerMan, String, Error>;

    fn handle(
        &mut self,
        msg: DestroySession,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<DestroySession>>::Result {
        let container_id = msg.session_id.into();

        let api = match self.docker_api {
            Some(ref api) => api,
            _ => return ActorResponse::reply(Err(Error::UnknownEnv("docker".into()))),
        };

        ActorResponse::async(
            api.container(container_id)
                .delete()
                .map_err(|_e| Error::Error("docker error".into()))
                .and_then(|_| Ok("done".into()))
                .into_actor(self),
        )
    }
}

struct Init;

impl gu_base::Module for Init {
    fn run<D: gu_base::Decorator + Clone + 'static>(&self, _decorator: D) {
        gu_base::run_once(|| {
            let _ = DockerMan::default().start();
        });
    }
}

pub fn module() -> impl gu_base::Module {
    Init
}
