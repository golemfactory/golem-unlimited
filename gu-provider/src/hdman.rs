use super::{
    envman, status,
    sync_exec::{Exec, ExecResult, SyncExecManager},
};
use actix::{fut, prelude::*};
use actix_web::client;
use actix_web::error::ErrorInternalServerError;
use deployment::DeployManager;
use deployment::Destroy;
use deployment::IntoDeployInfo;
use futures::future;
use futures::prelude::*;
use gu_actix::prelude::*;
use gu_base::files::read_async;
use gu_model::envman::*;
use gu_net::rpc::{
    peer::{PeerSessionInfo, PeerSessionStatus},
    *,
};
use gu_persist::config::ConfigModule;
use id::generate_new_id;
use provision::download_step;
use provision::upload_step;
use provision::{download, untgz};
use std::collections::HashSet;
use std::sync::Arc;
use std::{collections::HashMap, fs, path::PathBuf, process, result, time};
use workspace::Workspace;

impl IntoDeployInfo for HdSessionInfo {
    fn convert(&self, id: &String) -> PeerSessionInfo {
        PeerSessionInfo {
            id: id.clone(),
            name: self.workspace.get_name().clone(),
            status: self.status.clone(),
            tags: self.workspace.get_tags(),
            note: self.note.clone(),
            processes: self.processes.keys().cloned().collect(),
        }
    }
}

impl Destroy for HdSessionInfo {
    fn destroy(&mut self) -> Result<(), Error> {
        debug!("killing all running child processes");
        let _ = self
            .processes
            .values_mut()
            .map(|child| child.kill())
            .collect::<Vec<_>>();
        let _ = self
            .processes
            .values_mut()
            .map(|child| child.wait())
            .collect::<Vec<_>>();
        self.workspace.clear_dir().map_err(From::from)
    }
}

/// Host direct manager
pub struct HdMan {
    deploys: DeployManager<HdSessionInfo>,
    cache_dir: PathBuf,
    sessions_dir: PathBuf,
}

impl envman::EnvManService for HdMan {
    type CreateOptions = ();
}

impl Actor for HdMan {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        envman::register("hd", ctx.address());

        status::StatusManager::from_registry().do_send(status::AddProvider::new(
            "hostDirect",
            ctx.address().recipient(),
        ));

        ctx.run_interval(time::Duration::from_secs(10), |act, _| {
            act.scan_for_processes()
        });
    }
}

impl HdMan {
    pub fn start(config: &ConfigModule) -> Addr<Self> {
        let cache_dir = config.cache_dir().to_path_buf().join("images");
        let sessions_dir = config.work_dir().to_path_buf().join("sessions");

        debug!(
            "creating dirs for:\nimage cache {:?}\nsessions:{:?}",
            cache_dir, sessions_dir
        );
        fs::create_dir_all(&cache_dir)
            .and_then(|_| fs::create_dir_all(&sessions_dir))
            .map_err(|e| error!("Cannot create HdMan dir: {:?}", e))
            .unwrap();

        start_actor(HdMan {
            deploys: Default::default(),
            cache_dir,
            sessions_dir,
        })
    }

    fn get_session_path(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(session_id)
    }

    fn get_cache_path(&self, file_name: &str) -> PathBuf {
        self.cache_dir.join(file_name)
    }

    fn get_session_exec_path(&self, session_id: &str, executable: &str) -> String {
        self.get_session_path(session_id)
            .join(executable.trim_left_matches('/'))
            .into_os_string()
            .into_string()
            .unwrap()
    }

    fn get_session_mut(&mut self, session_id: &str) -> Result<&mut HdSessionInfo, Error> {
        match self.deploys.deploy_mut(session_id) {
            Ok(session) => Ok(session),
            Err(_) => Err(Error::NoSuchSession(session_id.into())),
        }
    }

    fn insert_child(&mut self, session_id: &str, child: process::Child) -> Result<String, Error> {
        Ok(self.get_session_mut(&session_id)?.insert_process(child))
    }

    fn scan_for_processes(&mut self) {
        for sess_info in self.deploys.values_mut() {
            let finished: Vec<String> = sess_info
                .processes
                .iter_mut()
                .filter_map(|(id, child)| match child.try_wait() {
                    Ok(Some(_exit_st)) => Some(id.clone()),
                    _ => None,
                })
                .collect();

            let some_finished = !finished.is_empty();
            for f in finished {
                sess_info.processes.remove(&f);
                info!("finished {:?}; removing", f)
            }

            if some_finished & sess_info.processes.is_empty() {
                sess_info.status = PeerSessionStatus::CONFIGURED;
            }
        }
    }
}

/// internal session representation
struct HdSessionInfo {
    workspace: Workspace,
    status: PeerSessionStatus,
    /// used to determine proper status when last child is finished
    dirty: bool,
    note: Option<String>,
    processes: HashMap<String, process::Child>,
}

impl HdSessionInfo {
    fn insert_process(&mut self, child: process::Child) -> String {
        let id = generate_new_id(&self.processes);
        self.processes.insert(id.clone(), child);
        self.dirty = true;
        self.status = PeerSessionStatus::RUNNING;
        id
    }
}

impl Handler<CreateSession> for HdMan {
    type Result = ActorResponse<HdMan, String, Error>;

    fn handle(
        &mut self,
        msg: CreateSession,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<CreateSession>>::Result {
        let session_id = self.deploys.generate_session_id();
        let work_dir = self.get_session_path(&session_id);
        let cache_path = self.get_cache_path(&msg.image.hash);

        let mut workspace = Workspace::new(msg.name, work_dir.clone());
        workspace.add_tags(msg.tags);
        match workspace.create_dirs() {
            Ok(_) => (),
            Err(e) => return ActorResponse::reply(Err(e.into())),
        }

        let session = HdSessionInfo {
            workspace,
            status: PeerSessionStatus::PENDING,
            dirty: false,
            note: msg.note,
            processes: HashMap::new(),
        };

        debug!("newly created session id={}", session_id);
        self.deploys.insert_deploy(session_id.clone(), session);

        debug!("hey! I'm downloading from: {:?}", msg.image);
        let sess_id = session_id.clone();
        ActorResponse::async(
            download(msg.image.url.as_ref(), cache_path.clone(), true)
                .map_err(From::from)
                .and_then(move |_| untgz(cache_path, work_dir))
                .map_err(From::from)
                .into_actor(self)
                .and_then(|_, act, _ctx| match act.get_session_mut(&sess_id) {
                    Ok(session) => {
                        session.status = PeerSessionStatus::CREATED;
                        fut::ok(sess_id)
                    }
                    Err(e) => fut::err(e),
                })
                .map_err(
                    move |e, act, _ctx| match act.deploys.destroy_deploy(&session_id) {
                        Ok(_) => Error::IoError(format!("creating session error: {:?}", e)),
                        Err(e) => e,
                    },
                ),
        )
    }
}

fn run_command(
    hd_man: &mut HdMan,
    session_id: String,
    command: Command,
) -> Box<ActorFuture<Actor = HdMan, Item = String, Error = String>> {
    match command {
        Command::Open { args } => Box::new(fut::ok("Open mock".to_string())),
        Command::Close => Box::new(fut::ok("Close mock".to_string())),
        Command::Exec { executable, args } => {
            let executable = hd_man.get_session_exec_path(&session_id, &executable);
            let session_id = session_id.clone();
            let session_dir = hd_man.get_session_path(&session_id).to_owned();

            info!("executing sync: {} {:?}", executable, args);
            Box::new(
                SyncExecManager::from_registry()
                    .send(Exec::Run {
                        executable,
                        args,
                        cwd: session_dir.clone(),
                    })
                    .flatten_fut()
                    .map_err(move |e| e.to_string())
                    .into_actor(hd_man)
                    .and_then(move |res, act, _ctx| {
                        info!("sync cmd result: {:?}", res);
                        let result = if let ExecResult::Run(output) = res {
                            String::from_utf8_lossy(&output.stdout).to_string()
                        } else {
                            "".to_string()
                        };

                        match act.get_session_mut(&session_id) {
                            Ok(session) => {
                                session.dirty = true;
                                fut::ok(result)
                            }
                            Err(e) => fut::err(e.to_string()),
                        }
                    }),
            )
        }
        Command::Start { executable, args } => {
            let executable = hd_man.get_session_exec_path(&session_id, &executable);

            info!("executing async: {} {:?}", executable, args);
            // TODO: critical section
            // TODO: env::set_current_dir(&base_dir)?;

            let child_res = process::Command::new(&executable)
                .args(&args)
                .spawn()
                .map_err(|e| Error::IoError(e.to_string()))
                .and_then(|child| hd_man.insert_child(&session_id, child));

            Box::new(match child_res {
                Ok(id) => fut::ok(id),
                Err(e) => fut::err(e.to_string()),
            })
        }
        Command::Stop { child_id } => {
            let session_id = session_id.clone();
            info!("killing: {:?}", &child_id);

            let kill_res = hd_man
                .get_session_mut(&session_id)
                .map_err(|e| e.to_string())
                .and_then(|session| {
                    session
                        .processes
                        .remove(&child_id)
                        .ok_or(Error::NoSuchChild(child_id).to_string())
                });

            Box::new(
                fut::result(kill_res).and_then(move |child, act: &mut HdMan, _ctx| {
                    SyncExecManager::from_registry()
                        .send(Exec::Kill(child))
                        .map_err(|e| format!("{}", e))
                        .and_then(|r| {
                            if let Ok(ExecResult::Kill(output)) = r {
                                Ok(output)
                            } else {
                                Err(format!("wrong result {:?}", r))
                            }
                        })
                        .into_actor(act)
                        .and_then(move |output, act, _ctx| {
                            fut::result(
                                act.get_session_mut(&session_id)
                                    .map(|session| {
                                        if session.processes.is_empty() {
                                            session.status = PeerSessionStatus::CONFIGURED;
                                        };
                                        output
                                    })
                                    .map_err(|e| e.to_string()),
                            )
                        })
                }),
            )
        }
        Command::DownloadFile {
            uri,
            file_path,
            format,
        } => {
            let path = hd_man.get_session_path(&session_id).join(file_path);
            Box::new(handle_download_file(uri, path, format).into_actor(hd_man))
        }
        Command::UploadFile {
            uri,
            file_path,
            format,
        } => {
            let path = hd_man.get_session_path(&session_id).join(file_path);
            Box::new(handle_upload_file(uri, path, format).into_actor(hd_man))
        }
        Command::AddTags(tags) => Box::new(fut::result(
            hd_man
                .get_session_mut(&session_id)
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
            hd_man
                .get_session_mut(&session_id)
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
    hd_man: &mut HdMan,
    session_id: String,
    commands: Vec<Command>,
) -> impl ActorFuture<Actor = HdMan, Item = Vec<String>, Error = Vec<String>> {
    let f: Box<dyn ActorFuture<Actor = HdMan, Item = Vec<String>, Error = Vec<String>>> =
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

impl Handler<SessionUpdate> for HdMan {
    /// ok: succeeded cmds output
    /// err: all succeeded cmds output till first failure, plus failed cmd err msg
    type Result = ActorResponse<HdMan, Vec<String>, Vec<String>>;

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

fn handle_download_file(
    uri: String,
    file_path: PathBuf,
    _format: ResourceFormat,
) -> impl Future<Item = String, Error = String> {
    download(uri.as_ref(), file_path, false)
        .and_then(move |_| Ok(format!("{:?} file downloaded", uri)))
        .map_err(|e| e.to_string())
}

fn handle_upload_file(
    uri: String,
    file_path: PathBuf,
    _format: ResourceFormat,
) -> impl Future<Item = String, Error = String> {
    future::result(
        client::put(uri.clone())
            .streaming(read_async(file_path).map_err(|e| ErrorInternalServerError(e))),
    )
    .map_err(|e| e.to_string())
    .and_then(|req| req.send().map_err(|e| e.to_string()))
    .and_then(move |res| {
        if res.status().is_success() {
            Ok(format!("{:?} file uploaded", uri))
        } else {
            Err(format!("Unsuccessful file upload: {}", res.status()))
        }
    })
    .map_err(|e| e.to_string())
}

// TODO: implement child process polling and status reporting
#[derive(Serialize, Deserialize, Debug)]
struct SessionStatus {
    session_id: String,
}

impl Handler<GetSessions> for HdMan {
    type Result = result::Result<Vec<PeerSessionInfo>, ()>;

    fn handle(&mut self, _msg: GetSessions, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.deploys.deploys_info())
    }
}

impl Handler<DestroySession> for HdMan {
    type Result = ActorResponse<HdMan, String, Error>;

    fn handle(
        &mut self,
        msg: DestroySession,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<DestroySession>>::Result {
        ActorResponse::async(match self.deploys.destroy_deploy(&msg.session_id) {
            Ok(_) => fut::ok("Session closed".into()),
            Err(e) => fut::err(e),
        })
    }
}

impl Handler<status::GetEnvStatus> for HdMan {
    type Result = MessageResult<status::GetEnvStatus>;

    fn handle(
        &mut self,
        _msg: status::GetEnvStatus,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<status::GetEnvStatus>>::Result {
        MessageResult(self.deploys.status())
    }
}

impl HdMan {
    fn command_start(
        &mut self,
        session_id: Arc<String>,
        args: Vec<String>,
        ctx: &mut <Self as Actor>::Context,
    ) -> ActorResponse<Self, String, String> {
        ActorResponse::reply(Ok("Opened".into()))
    }

    fn command_stop(&mut self, session_id: Arc<String>) -> ActorResponse<Self, String, String> {
        ActorResponse::reply(Ok("Closed".into()))
    }
}
