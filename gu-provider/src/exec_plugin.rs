use crate::deployment::{DeployManager, Destroy, IntoDeployInfo};
use crate::workspace::{Workspace, WorkspacesManager};
use crate::{envman, status};
use actix::prelude::*;
use gu_hdman::process_pool::{KillAll, ProcessPool};
use gu_model::envman::{CreateSession, DestroySession, GetSessions, SessionUpdate};
use gu_model::plugin::{SimpleExecEnvSpec, PluginManifest};
use gu_net::rpc::RemotingContext;
use std::path::{PathBuf, Path};
use std::process;
use tokio_process::CommandExt;
use std::{io, fs};

use crate::envman::EnvManService;
use crate::status::GetEnvStatus;
use futures::Future;
use gu_hdman::image_manager;
use gu_model::envman::Error as EnvError;
use gu_net::rpc::peer::{PeerSessionInfo, PeerSessionStatus};
use gu_persist::config::ConfigModule;
use std::fs::OpenOptions;
use gu_base::{Module, Decorator};

pub struct PluginMan {
    code: String,
    exec: PathBuf,
    deploys: DeployManager<PlugSession>,
    workspaces_man: WorkspacesManager,
}

impl PluginMan {
    fn from_spec(base_path : &Path, spec: SimpleExecEnvSpec, config: &ConfigModule) -> Self {
        let code = spec.code;
        let exec = base_path.join(spec.exec);
        let deploys = Default::default();
        let workspaces_man = WorkspacesManager::new(config, code.clone()).unwrap();
        PluginMan {
            code,
            exec,
            deploys,
            workspaces_man,
        }
    }
}

impl Actor for PluginMan {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        envman::register(std::borrow::Cow::Owned(self.code.clone()), ctx.address());

        status::StatusManager::from_registry().do_send(status::AddProvider::new(
            self.code.clone(),
            ctx.address().recipient(),
        ));
    }
}

impl EnvManService for PluginMan {
    type CreateOptions = serde_json::Value;
}

struct PlugSession {
    workspace: Workspace,
    exec: PathBuf,
    image_path: PathBuf,
    spec_path: PathBuf,
    pool: Addr<ProcessPool>,
}

impl IntoDeployInfo for PlugSession {
    fn convert(&self, id: &String) -> PeerSessionInfo {
        // TODO: Implement this
        let id = id.to_owned();
        let name = "".to_owned();
        let status = PeerSessionStatus::CONFIGURED;
        let tags = Default::default();
        let note = None;
        let processes = Default::default();

        PeerSessionInfo {
            id,
            name,
            status,
            tags,
            note,
            processes,
        }
    }
}

impl Destroy for PlugSession {
    fn destroy(&mut self) -> Box<Future<Item = (), Error = EnvError>> {
        Box::new(
            self.pool
                .send(KillAll)
                .map_err(|e| EnvError::Error(format!("{}", e)))
                .flatten(),
        )
    }
}

impl Handler<CreateSession<<Self as EnvManService>::CreateOptions>> for PluginMan {
    type Result = ActorResponse<PluginMan, String, EnvError>;

    fn handle(
        &mut self,
        msg: CreateSession<<Self as EnvManService>::CreateOptions>,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<CreateSession<<Self as EnvManService>::CreateOptions>>>::Result {
        // Download image
        let image_path = image_manager::image(msg.image)
            .map_err(|e| EnvError::IoError(format!("image pull error: {}", e)));

        let tags = msg.tags;
        let options = msg.options;

        ActorResponse::r#async(
            image_path
                .into_actor(self)
                .and_then(|image_path, act, _ctx| {
                    let async_status = match process::Command::new(&act.exec)
                        .args(&["validate-image".as_ref(), image_path.as_path().as_os_str()])
                        .status_async()
                    {
                        Ok(s) => s.map_err(|e| EnvError::Error(e.to_string())),
                        Err(e) => return fut::Either::B(fut::err(EnvError::Error(e.to_string()))),
                    };
                    fut::Either::A(
                        async_status
                            .and_then(|status| {
                                if status.success() {
                                    Ok(image_path)
                                } else {
                                    Err(EnvError::Error("invalid image".into()))
                                }
                            })
                            .into_actor(act),
                    )
                })
                .map_err(|e, _, _| EnvError::Error(format!("{}", e)))
                .and_then(move |image_path, act, ctx| {
                    // Image is valid
                    let mut workspace = act.workspaces_man.workspace();
                    workspace.add_tags(tags);
                    if let Err(e) = workspace.create_dirs() {
                        return fut::Either::B(fut::err(EnvError::IoError(e.to_string())));
                    }
                    let exec = act.exec.clone();
                    let workspace_path = workspace.path().clone();
                    let spec_path = workspace_path.join("deployment-spec.json");
                    let session_id = act.deploys.generate_session_id();
                    let pool = ProcessPool::with_work_dir(&workspace_path).start();

                    let json = match serde_json::to_vec_pretty(&options) {
                        Ok(json) => json,
                        Err(e) => {
                            return fut::Either::B(fut::err(EnvError::IncorrectOptions(
                                e.to_string(),
                            )))
                        }
                    };
                    if let Err(e) = std::fs::write(&spec_path, json) {
                        return fut::Either::B(fut::err(EnvError::IoError(e.to_string())));
                    }
                    let create_result = match process::Command::new(&act.exec)
                        .args(&[
                            "create".as_ref(),
                            "--image".as_ref(),
                            image_path.as_os_str(),
                            "--workdir".as_ref(),
                            workspace_path.as_os_str(),
                            "--spec".as_ref(),
                            spec_path.as_os_str(),
                        ])
                        .status_async()
                        .map_err(EnvError::from)
                    {
                        Ok(s) => s,
                        Err(e) => {
                            return fut::Either::B(fut::err(EnvError::IoError(e.to_string())))
                        }
                    };

                    fut::Either::A(
                        create_result
                            .map_err(EnvError::from)
                            .into_actor(act)
                            .and_then(move |s, act, _| {
                                act.deploys.insert_deploy(
                                    session_id.clone(),
                                    PlugSession {
                                        workspace,
                                        exec,
                                        image_path,
                                        spec_path,
                                        pool,
                                    },
                                );
                                fut::ok(session_id)
                            }),
                    )
                }),
        )
    }
}

impl Handler<SessionUpdate> for PluginMan {
    type Result = ActorResponse<Self, Vec<String>, Vec<String>>;

    fn handle(&mut self, msg: SessionUpdate, ctx: &mut Self::Context) -> Self::Result {
        unimplemented!()
    }
}

impl Handler<GetSessions> for PluginMan {
    type Result = Result<Vec<PeerSessionInfo>, ()>;

    fn handle(&mut self, msg: GetSessions, ctx: &mut Self::Context) -> Self::Result {
        Ok(self.deploys.deploys_info())
    }
}

impl Handler<DestroySession> for PluginMan {
    type Result = ActorResponse<Self, String, EnvError>;

    fn handle(&mut self, msg: DestroySession, ctx: &mut Self::Context) -> Self::Result {
        ActorResponse::r#async(
            self.deploys
                .destroy_deploy(&msg.session_id)
                .into_actor(self)
                .then(|r, _act, _ctx| match r {
                    Ok(_) => fut::ok("Session closed".into()),
                    Err(e) => fut::err(e),
                }),
        )
    }
}

impl Handler<status::GetEnvStatus> for PluginMan {
    type Result = MessageResult<status::GetEnvStatus>;

    fn handle(&mut self, msg: GetEnvStatus, ctx: &mut Self::Context) -> Self::Result {
        MessageResult(self.deploys.status())
    }
}


struct ExecPlugModule;

impl Module for ExecPlugModule {
    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        let config: &ConfigModule = decorator.extract().unwrap();
        let work_dir = config.work_dir();
        let _ = scan_for_plugins(&work_dir, config).unwrap();
    }
}


fn scan_for_plugins(work_dir : &Path, config : &ConfigModule) -> io::Result<()> {
    for item in fs::read_dir(work_dir.join("plugins"))? {
        match item {
            Ok(ent) => {
                let path = ent.path();
                let ftype = ent.file_type()?;
                if ftype.is_dir() && ent.path().extension() == Some("gu-plugin".as_ref()) {
                    eprintln!("plugin: {}", path.display());
                    let manifest : PluginManifest = serde_json::from_reader(fs::File::open(path.join("gu-plugin.json"))?)
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    if let Some(p) = manifest.provider {
                        for activator in p.simple_exec_env {
                            let _ = PluginMan::from_spec(&path, activator, config).start();
                        }
                    }

                }
                else {
                    eprintln!("not plugin: {}", path.display());
                }

            },
            Err(_) => ()
        }
    }
    Ok(())
}

pub fn module() -> impl Module {
    ExecPlugModule
}