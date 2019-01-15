//! Docker mode implementation

use super::envman;
use actix::prelude::*;
use async_docker::models::ContainerConfig;
use async_docker::{self, new_docker, DockerApi};
use deployment::DeployManager;
use deployment::Destroy;
use deployment::IntoDeployInfo;
use futures::future;
use futures::prelude::*;
use gu_model::dockerman::CreateOptions;
use gu_model::envman::*;
use gu_net::rpc::peer::PeerSessionInfo;
use gu_net::rpc::peer::PeerSessionStatus;
use std::collections::BTreeMap;
use std::collections::HashSet;
use workspace::Workspace;

// Actor.
#[derive(Default)]
struct DockerMan {
    docker_api: Option<Box<DockerApi>>,
    deploys: DeployManager<DockerSessionInfo>,
}

struct DockerSessionInfo {
    workspace: Workspace,
}

impl IntoDeployInfo for DockerSessionInfo {
    fn convert(&self, id: &String) -> PeerSessionInfo {
        PeerSessionInfo {
            id: id.clone(),
            name: self.workspace.get_name().clone(),
            status: PeerSessionStatus::CREATED,
            tags: self.workspace.get_tags(),
            note: None,
            processes: HashSet::new(),
        }
    }
}

impl Destroy for DockerSessionInfo {}

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

fn run_command(
    docker_man: &mut DockerMan,
    session_id: String,
    command: Command,
) -> Box<ActorFuture<Actor = DockerMan, Item = String, Error = String>> {
    match command {
        Command::Open { args } => Box::new(fut::ok("Open mock".to_string())),
        Command::Close => Box::new(fut::ok("Close mock".to_string())),
        Command::Exec { executable, args } => Box::new(fut::ok("Exec mock".to_string())),
        Command::Start { executable, args } => Box::new(fut::ok("Start mock".to_string())),
        Command::Stop { child_id } => Box::new(fut::ok("Stop mock".to_string())),
        Command::DownloadFile {
            uri,
            file_path,
            format,
        } => Box::new(fut::ok("Download mock".to_string())),
        Command::UploadFile {
            uri,
            file_path,
            format,
        } => Box::new(fut::ok("Upload mock".to_string())),
        Command::AddTags(tags) => Box::new(fut::result(
            docker_man
                .deploys
                .deploy_mut(&session_id)
                .map(|session| {
                    session.workspace.add_tags(tags);
                    format!(
                        "tags inserted. Current tags are: {:?}",
                        &session.workspace.get_tags()
                    )
                })
                .map_err(|e| e.to_string()),
        )),
        Command::DelTags(tags) => Box::new(fut::result(
            docker_man
                .deploys
                .deploy_mut(&session_id)
                .map(|session| {
                    session.workspace.remove_tags(tags);
                    format!(
                        "tags inserted. Current tags are: {:?}",
                        &session.workspace.get_tags()
                    )
                })
                .map_err(|e| e.to_string()),
        )),
    }
}

fn run_commands(
    hd_man: &mut DockerMan,
    session_id: String,
    commands: Vec<Command>,
) -> impl ActorFuture<Actor = DockerMan, Item = Vec<String>, Error = Vec<String>> {
    let f: Box<dyn ActorFuture<Actor = DockerMan, Item = Vec<String>, Error = Vec<String>>> =
        Box::new(future::ok(Vec::new()).into_actor(hd_man));

    commands.into_iter().fold(f, |acc, command| {
        let session_id = session_id.clone();
        Box::new(acc.and_then(|mut vec, act, _ctx| {
            run_command(act, session_id, command).then(move |i, _, _| match i {
                Ok(a) => {
                    vec.push(a);
                    fut::ok(vec)
                }
                Err(a) => {
                    vec.push(a);
                    fut::err(vec)
                }
            })
        }))
    })
}

impl Handler<SessionUpdate> for DockerMan {
    type Result = ActorResponse<DockerMan, Vec<String>, Vec<String>>;

    fn handle(&mut self, msg: SessionUpdate, _ctx: &mut Self::Context) -> Self::Result {
        if !self.deploys.contains_deploy(&msg.session_id) {
            return ActorResponse::reply(Err(
                vec![Error::NoSuchSession(msg.session_id).to_string()],
            ));
        }
        let session_id = msg.session_id.clone();

        ActorResponse::async(run_commands(self, session_id, msg.commands))
    }
}

impl Handler<GetSessions> for DockerMan {
    type Result = ActorResponse<DockerMan, Vec<PeerSessionInfo>, ()>;

    fn handle(
        &mut self,
        _msg: GetSessions,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<GetSessions>>::Result {
        ActorResponse::reply(Ok(self.deploys.deploys_info()))
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
