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
use gu_model::dockerman::{CreateOptions, VolumeDef};
use gu_model::envman::*;
use gu_net::rpc::peer::PeerSessionInfo;
use gu_net::rpc::peer::PeerSessionStatus;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::PathBuf;
use workspace::Workspace;

// Actor.
#[derive(Default)]
struct DockerMan {
    docker_api: Option<Box<DockerApi>>,
    deploys: DeployManager<DockerSession>,
}

struct DockerSession {
    workspace: Workspace,
    container: Option<async_docker::models::Container>,
    status: PeerSessionStatus,
}

impl DockerSession {

    fn do_open(&mut self, session_id : String, docker_api : &DockerApi) -> impl Future<Item=String, Error=String> {
        docker_api.container(session_id.into()).start().then(|r| {
            match r {
                Ok(status) => Ok("OK".into()),
                Err(e) => Err(format!("{}", e))
            }
        })
    }

    fn do_close(&mut self, session_id : String, docker_api : &DockerApi) -> impl Future<Item=String, Error=String> {
        docker_api.container(Cow::Owned(session_id)).stop(None)
            .map_err(|e| format!("{}", e))
            .and_then(|v| Ok("OK".into()))
    }

    fn do_exec(&mut self, session_id : String, docker_api : &DockerApi, executable : String, mut args : Vec<String>) -> impl Future<Item=String, Error=String> {
        args.insert(0, executable);
        let cfg = {
            use async_docker::models::*;

            ExecConfig::new().with_attach_stdout(true)
                .with_attach_stderr(true)
                .with_cmd(args)
        };


        docker_api.container(Cow::Owned(session_id)).exec(&cfg)
            .map_err(|e| format!("{}", e))
            .fold(String::new(), |mut s, (t, it)| {
                use std::str;

                match str::from_utf8(it.into_bytes().as_ref()) {
                    Ok(chunk_str) => s.push_str(chunk_str),
                    Err(_) => ()
                };

                Ok::<String, String>(s)
            })

    }

}

impl IntoDeployInfo for DockerSession {
    fn convert(&self, id: &String) -> PeerSessionInfo {
        PeerSessionInfo {
            id: id.clone(),
            name: self.workspace.name().clone(),
            status: PeerSessionStatus::CREATED,
            tags: self.workspace.tags(),
            note: None,
            processes: HashSet::new(),
        }
    }
}

impl Destroy for DockerSession {}

impl DockerMan {
    fn container_config(
        url: String,
        host_config: async_docker::models::HostConfig,
    ) -> ContainerConfig {
        ContainerConfig::new()
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
            .with_host_config(host_config)
    }
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

                let binds: Vec<String> = msg
                    .options
                    .volumes
                    .iter()
                    .filter_map(|vol: &VolumeDef| {
                        vol.source_dir()
                            .and_then(|s| vol.target_dir().map(|t| (s, t)))
                            .map(|(s, t)| format!("{}:{}", s, t))
                    })
                    .collect();
                let host_config = async_docker::models::HostConfig::new().with_binds(binds);

                //let docker_image = api.image(url.as_ref().into());
                let opts = Self::container_config(url.into(), host_config);

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


impl DockerMan {

    fn run_for_deployment<F,R>(&mut self, deployment_id : String, f : F) -> Box<ActorFuture<Actor=DockerMan, Item=String, Error=String>>
        where F : FnOnce(&mut DockerSession, String, &DockerApi) -> R, R : Future<Item=String, Error=String> + 'static {
        let deployment = match self.deploys.deploy_mut(&deployment_id) {
            Ok(deployment) => deployment,
            Err(e) => return Box::new(fut::err(format!("{}", e)))
        };

        let docker_api = self.docker_api.as_ref().unwrap().as_ref();

        Box::new(fut::wrap_future(f(deployment, deployment_id, docker_api)))
    }

}


fn run_command(
    docker_man: &mut DockerMan,
    session_id: String,
    command: Command,
) -> Box<ActorFuture<Actor = DockerMan, Item = String, Error = String>> {

    if docker_man.docker_api.is_none() {
        return Box::new(fut::err("Docker API not initialized properly".to_string()));
    }

    match command {
        Command::Open => docker_man.run_for_deployment(session_id, DockerSession::do_open),
        Command::Close => docker_man.run_for_deployment(session_id, DockerSession::do_close),
        Command::Exec { executable, args } => docker_man.run_for_deployment(session_id, |deployment, deployment_id, docker_api| {
            deployment.do_exec(deployment_id, docker_api, executable, args)
        }),
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
                        &session.workspace.tags()
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
                        "tags removed. Current tags are: {:?}",
                        &session.workspace.tags()
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
