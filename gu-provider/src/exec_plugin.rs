use crate::deployment::{DeployManager, Destroy, IntoDeployInfo};
use crate::workspace::{Workspace, WorkspacesManager};
use crate::{envman, status};
use actix::prelude::*;
use gu_hdman::process_pool::{self as pp, KillAll, ProcessPool};
use gu_model::envman::{Command, CreateSession, DestroySession, GetSessions, SessionUpdate};
use gu_model::plugin::{PluginManifest, ResolveResult, SimpleExecEnvSpec};
use std::path::{Path, PathBuf};
use std::process;
use std::{fs, io};
use tokio_process::CommandExt;

use crate::envman::EnvManService;
use crate::status::GetEnvStatus;
use futures::{Future, IntoFuture};
use gu_base::{Decorator, Module};
use gu_hdman::image_manager;
use gu_model::envman::Error as EnvError;
use gu_net::rpc::peer::{PeerSessionInfo, PeerSessionStatus};
use gu_persist::config::ConfigModule;
use std::fs::OpenOptions;

pub struct PluginMan {
    code: String,
    exec: PathBuf,
    deploys: DeployManager<PlugSession>,
    workspaces_man: WorkspacesManager,
}

impl PluginMan {
    fn from_spec(base_path: &Path, spec: SimpleExecEnvSpec, config: &ConfigModule) -> Self {
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
    fn destroy(&mut self) -> Box<dyn Future<Item = (), Error = EnvError>> {
        /// TODO Add self.workspace.clear_dir().map_err(From::from).into_future()
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
                    let pool = ProcessPool::with_work_dir(&workspace_path)
                        .with_exec(&exec)
                        .start();

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
        let session = match self.deploys.deploy(&msg.session_id) {
            Ok(v) => v,
            Err(e) => return ActorResponse::reply(Err(vec![e.to_string()])),
        };
        let session_id = msg.session_id;
        let exec = session.exec.clone();
        let image_path = session.image_path.clone();
        let work_dir = session.workspace.path().clone();
        let spec_path = session.clone().spec_path.clone();
        let pool = session.pool.clone();

        ActorResponse::r#async(crate::fchain::process_chain_act(
            self,
            ctx,
            msg.commands,
            move |command, act, ctx| {
                match command {
                    Command::AddTags(new_tags) => {
                        if let Ok(session) = act.deploys.deploy_mut(&session_id) {
                            session.workspace.add_tags(new_tags);
                            Box::new(futures::future::ok(format!(
                                "tags added. Current tags are: {:?}",
                                &session.workspace.tags()
                            )))
                        } else {
                            Box::new(futures::future::err("session closed".into()))
                        }
                    }
                    Command::DelTags(tags) => {
                        if let Ok(session) = act.deploys.deploy_mut(&session_id) {
                            session.workspace.remove_tags(tags);
                            Box::new(futures::future::ok(format!(
                                "tags deleted. Current tags are: {:?}",
                                &session.workspace.tags()
                            )))
                        } else {
                            Box::new(futures::future::err("session closed".into()))
                        }
                    }

                    Command::Open => {
                        pool.do_send(pp::Exec {
                            executable: exec.clone(),
                            args: vec![
                                "open".into(),
                                "--image".into(),
                                image_path.to_string_lossy().into(),
                                "--workdir".into(),
                                work_dir.to_string_lossy().into(),
                                "--spec".into(),
                                spec_path.to_string_lossy().into(),
                            ],
                        });
                        Box::new(futures::future::ok("Ok".into()))
                    }
                    Command::Exec {
                        executable,
                        mut args,
                        /*TODO */ working_dir,
                    } => {
                        let mut driver_args: Vec<String> = vec![
                            "exec".into(),
                            "--image".into(),
                            image_path.to_string_lossy().into(),
                            "--workdir".into(),
                            work_dir.to_string_lossy().into(),
                            "--spec".into(),
                            spec_path.to_string_lossy().into(),
                            "--".into(),
                            executable,
                        ];
                        driver_args.append(&mut args);

                        Box::new(
                            pool.send(pp::Exec {
                                executable: exec.clone(),
                                args: driver_args,
                            })
                            .map_err(|_e| "process pool destroyed".into())
                            .and_then(|r| r)
                            .and_then(|(stdout, _stderr)| Ok(stdout)),
                        )
                    }
                    Command::DownloadFile {
                        uri,
                        file_path,
                        format,
                    } => Box::new(
                        resolve_path(
                            &exec,
                            &image_path,
                            &work_dir,
                            &spec_path,
                            file_path.as_ref(),
                        )
                        .and_then(move |resp: ResolveResult| match resp {
                            ResolveResult::ResolvedPath(output_path) => {
                                crate::provision::download_step(&uri, output_path.into(), format)
                            }
                        })
                        .and_then(|_| Ok("downloaded".into())),
                    ),
                    Command::WriteFile { content, file_path } => Box::new(
                        resolve_path(
                            &exec,
                            &image_path,
                            &work_dir,
                            &spec_path,
                            file_path.as_ref(),
                        )
                        .and_then(|resp| match resp {
                            ResolveResult::ResolvedPath(output_path) => {
                                fs::write(output_path, content).map_err(|e| e.to_string())?;
                                Ok("OK".to_string())
                            }
                        }),
                    ),
                    Command::UploadFile {
                        uri,
                        file_path,
                        format,
                    } => Box::new(
                        resolve_path(
                            &exec,
                            &image_path,
                            &work_dir,
                            &spec_path,
                            file_path.as_ref(),
                        )
                        .and_then(move |resp: ResolveResult| match resp {
                            ResolveResult::ResolvedPath(input_path) => {
                                crate::provision::upload_step(&uri, input_path.into(), format)
                            }
                        })
                        .and_then(|_| Ok("uploaded".into())),
                    ),
                    Command::Close => {
                        Box::new(futures::future::err("Close not implemented".into()))
                    }
                    Command::Start { .. } => {
                        Box::new(futures::future::err("start not implemented".into()))
                    }
                    Command::Wait { .. } => {
                        Box::new(futures::future::err("wait not implemented".into()))
                    }
                    Command::Stop { child_id } => {
                        let pid: pp::Pid = match child_id.parse() {
                            Ok(pid) => pid,
                            Err(e) => return Box::new(futures::future::err(e.to_string())),
                        };
                        Box::new(
                            pool.send(pp::Stop(pid))
                                .map_err(|_| "process pool closed".into())
                                .and_then(|r| r)
                                .and_then(|_| Ok("killed".into())),
                        )
                    }
                }
            },
        ))
    }
}

fn resolve_path(
    exec: &Path,
    image_path: &Path,
    work_dir: &Path,
    spec_path: &Path,
    file_path: &Path,
) -> impl Future<Item = ResolveResult, Error = String> {
    process::Command::new(exec)
        .args(&[
            "resolve-path".as_ref(),
            "--image".as_ref(),
            image_path.as_os_str(),
            "--workdir".as_ref(),
            work_dir.as_os_str(),
            "--spec".as_ref(),
            spec_path.as_os_str(),
            file_path.as_ref(),
        ])
        .output_async()
        .map_err(|e| format!("driver error: {}", e))
        .and_then(|output| {
            eprintln!(
                "stderr={}",
                std::str::from_utf8(output.stderr.as_ref()).unwrap()
            );
            eprintln!(
                "output={}",
                std::str::from_utf8(output.stdout.as_ref()).unwrap()
            );
            serde_json::from_slice(output.stdout.as_ref())
                .map_err(|e| format!("driver error: {}", e))
        })
}

impl Handler<GetSessions> for PluginMan {
    type Result = Result<Vec<PeerSessionInfo>, ()>;

    fn handle(&mut self, _: GetSessions, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.deploys.deploys_info())
    }
}

impl Handler<DestroySession> for PluginMan {
    type Result = ActorResponse<Self, String, EnvError>;

    fn handle(&mut self, msg: DestroySession, _ctx: &mut Self::Context) -> Self::Result {
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

    fn handle(&mut self, _: GetEnvStatus, _ctx: &mut Self::Context) -> Self::Result {
        MessageResult(self.deploys.status())
    }
}

struct ExecPlugModule;

impl Module for ExecPlugModule {
    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        let config: &ConfigModule = decorator.extract().unwrap();
        let work_dir = config.work_dir();
        let _ = scan_for_plugins("/var/lib/golemu".as_ref(), config)
            .unwrap_or_else(|e| log::debug!("on scan /var/lib/golemu/plugins: {}", e));
        let _ = scan_for_plugins("/usr/lib/golemu".as_ref(), &config)
            .unwrap_or_else(|e| log::debug!("on scan /usr/lib/golemu/plugins: {}", e));
        #[cfg(windows)]
        {
            let _ = scan_for_plugins("plugins".as_ref(), config);
        }
        #[cfg(target_os = "macos")]
        {
            let _ = scan_for_plugins(
                "/Applications/Golem Unlimited Provider.app/Contents/Resources".as_ref(),
                config,
            )
            .unwrap_or_else(|e| log::debug!("/Applications/ scanning: {}", e));
        }
        let _ = scan_for_plugins(&work_dir, config)
            .unwrap_or_else(|e| log::debug!("on scan {:?}/plugins: {}", work_dir, e));
    }
}

fn scan_for_plugins(work_dir: &Path, config: &ConfigModule) -> io::Result<()> {
    let plugins_dir = work_dir.join("plugins");
    for item in fs::read_dir(plugins_dir)? {
        match item {
            Ok(ent) => {
                let path = ent.path();
                let ftype = ent.file_type()?;
                if ftype.is_dir() && ent.path().extension() == Some("gu-plugin".as_ref()) {
                    eprintln!("plugin: {}", path.display());
                    let manifest: PluginManifest =
                        serde_json::from_reader(fs::File::open(path.join("gu-plugin.json"))?)
                            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    if let Some(p) = manifest.provider {
                        for activator in p.simple_exec_env {
                            let _ = PluginMan::from_spec(&path, activator, config).start();
                        }
                    }
                }
            }
            Err(_) => (),
        }
    }
    Ok(())
}

pub fn module() -> impl Module {
    ExecPlugModule
}
