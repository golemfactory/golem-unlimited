use super::status;
use super::sync_exec::{Exec, ExecResult, SyncExecManager};
use actix::fut;
use actix::prelude::*;
use futures::prelude::*;
use gu_actix::prelude::*;
use gu_envman_api::*;
use gu_net::rpc::peer::{PeerSessionInfo, PeerSessionStatus};
use gu_net::rpc::*;
use gu_persist::config::ConfigModule;
use id::generate_new_id;
use provision::{download, untgz};
use std::collections::HashMap;
use std::collections::HashSet;
use std::iter::FromIterator;
use std::path::PathBuf;
use std::{fmt, fs, io, process, result, time};

/// Host direct manager
pub struct HdMan {
    sessions: HashMap<String, SessionInfo>,
    cache_dir: PathBuf,
    sessions_dir: PathBuf,
}

impl Actor for HdMan {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.bind::<CreateSession>(CreateSession::ID);
        ctx.bind::<SessionUpdate>(SessionUpdate::ID);
        ctx.bind::<GetSessions>(GetSessions::ID);
        ctx.bind::<DestroySession>(DestroySession::ID);

        status::StatusManager::from_registry().do_send(status::AddProvider::new(
            "hostDirect",
            ctx.address().recipient(),
        ));

        ctx.run_interval(time::Duration::from_secs(10), |act, _| {
            act.scan_for_processes()
        });
    }
}

impl Drop for HdMan {
    fn drop(&mut self) {
        let _: Vec<Result<(), Error>> = self
            .sessions
            .values_mut()
            .map(SessionInfo::destroy)
            .collect();
        println!("HdMan stopped");
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
            sessions: HashMap::new(),
            cache_dir,
            sessions_dir,
        })
    }

    fn generate_session_id(&self) -> String {
        generate_new_id(&self.sessions)
    }

    fn get_session_path(&self, session_id: &String) -> PathBuf {
        self.sessions_dir.join(session_id)
    }

    fn get_cache_path(&self, file_name: &String) -> PathBuf {
        self.cache_dir.join(file_name)
    }

    fn get_session_exec_path(&self, session_id: &String, executable: &String) -> String {
        self.get_session_path(session_id)
            .join(executable.trim_left_matches('/'))
            .into_os_string()
            .into_string()
            .unwrap()
    }

    fn get_session_mut(&mut self, session_id: &String) -> Result<&mut SessionInfo, Error> {
        match self.sessions.get_mut(session_id) {
            Some(session) => Ok(session),
            None => Err(Error::NoSuchSession(session_id.clone())),
        }
    }

    fn insert_child(
        &mut self,
        session_id: &String,
        child: process::Child,
    ) -> Result<String, Error> {
        Ok(self.get_session_mut(&session_id)?.insert_process(child))
    }

    fn destroy_session(&mut self, session_id: &String) -> Result<(), Error> {
        self.sessions
            .remove(session_id)
            .ok_or(Error::NoSuchSession(session_id.clone()))
            .and_then(|mut s| SessionInfo::destroy(&mut s))
    }

    fn scan_for_processes(&mut self) {
        for sess_info in self.sessions.values_mut().into_iter() {
            let finished: Vec<String> = sess_info
                .processes
                .iter_mut()
                .filter_map(|(id, child)| match child.try_wait() {
                    Ok(Some(_exit_st)) => Some(id.clone()),
                    _ => None,
                }).collect();

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
struct SessionInfo {
    name: String,
    status: PeerSessionStatus,
    /// used to determine proper status when last child is finished
    dirty: bool,
    tags: HashSet<String>,
    note: Option<String>,
    work_dir: PathBuf,
    processes: HashMap<String, process::Child>,
}

impl SessionInfo {
    fn insert_process(&mut self, child: process::Child) -> String {
        let id = generate_new_id(&self.processes);
        self.processes.insert(id.clone(), child);
        self.dirty = true;
        self.status = PeerSessionStatus::RUNNING;
        id
    }

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
        debug!("cleaning session dir {:?}", self.work_dir);
        fs::remove_dir_all(&self.work_dir).map_err(From::from)
    }
}

impl Handler<CreateSession> for HdMan {
    type Result = ActorResponse<HdMan, String, Error>;

    fn handle(
        &mut self,
        msg: CreateSession,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<CreateSession>>::Result {
        let session_id = self.generate_session_id();
        let work_dir = self.get_session_path(&session_id);
        debug!("creating work dir {:?}", work_dir);
        match fs::create_dir(&work_dir) {
            Ok(_) => (),
            Err(e) => return ActorResponse::reply(Err(e.into())),
        }
        let cache_path = self.get_cache_path(&msg.image.hash);

        let session = SessionInfo {
            name: msg.name,
            status: PeerSessionStatus::PENDING,
            dirty: false,
            tags: HashSet::from_iter(msg.tags.into_iter()),
            note: msg.note,
            work_dir: work_dir.clone(),
            processes: HashMap::new(),
        };

        debug!("newly created session id={}", session_id);
        self.sessions.insert(session_id.clone(), session);

        debug!("hey! I'm downloading from: {:?}", msg.image);
        let sess_id = session_id.clone();
        ActorResponse::async(
            download(msg.image.url.as_ref(), cache_path.clone())
                .map_err(From::from)
                .and_then(move |_| untgz(&cache_path, &work_dir))
                .map_err(From::from)
                .into_actor(self)
                .and_then(|_, act, _ctx| match act.get_session_mut(&sess_id) {
                    Ok(session) => {
                        session.status = PeerSessionStatus::CREATED;
                        fut::ok(sess_id)
                    }
                    Err(e) => fut::err(e),
                }).map_err(move |e, act, _ctx| {
                    match act.destroy_session(&session_id) {
                        Ok(_) => Error::IoError(format!("creating session error: {:?}", e)),
                        Err(e) => e,
                    }
                }),
        )
    }
}

impl Handler<SessionUpdate> for HdMan {
    /// ok: succeeded cmds output
    /// err: all succeeded cmds output till first failure, plus failed cmd err msg
    type Result = ActorResponse<HdMan, Vec<String>, Vec<String>>;

    fn handle(&mut self, msg: SessionUpdate, _ctx: &mut Self::Context) -> Self::Result {
        if !self.sessions.contains_key(&msg.session_id) {
            return ActorResponse::reply(Err(vec![
                Error::NoSuchSession(msg.session_id).to_string(),
            ]));
        }

        let mut future_chain: Box<
            ActorFuture<Item = Vec<String>, Error = Vec<String>, Actor = Self>,
        > = Box::new(fut::ok(Vec::new()));

        for cmd in msg.commands {
            let session_id = msg.session_id.clone();

            match cmd {
                Command::Exec { executable, args } => {
                    let executable = self.get_session_exec_path(&session_id, &executable);
                    future_chain = Box::new(future_chain.and_then(move |mut v, act, _ctx| {
                        let mut vc = v.clone();
                        info!("executing sync: {} {:?}", executable, args);
                        SyncExecManager::from_registry()
                            .send(Exec::Run { executable, args })
                            .flatten_fut()
                            .map_err(|e| {
                                vc.push(e.to_string());
                                vc
                            }).into_actor(act)
                            .and_then(move |result, act, _ctx| {
                                info!("sync cmd result: {:?}", result);
                                if let ExecResult::Run(output) = result {
                                    v.push(String::from_utf8_lossy(&output.stdout).to_string());
                                }
                                match act.get_session_mut(&session_id) {
                                    Ok(session) => {
                                        session.dirty = true;
                                        fut::ok(v)
                                    }
                                    Err(e) => {
                                        v.push(e.to_string());
                                        fut::err(v)
                                    }
                                }
                            })
                    }));
                }
                Command::Start { executable, args } => {
                    let executable = self.get_session_exec_path(&session_id, &executable);
                    future_chain = Box::new(future_chain.and_then(move |mut v, act, _ctx| {
                        info!("executing async: {} {:?}", executable, args);
                        // TODO: critical section
                        // TODO: env::set_current_dir(&base_dir)?;
                        let mut vc = v.clone();
                        process::Command::new(&executable)
                            .args(&args)
                            .spawn()
                            .map_err(|e| Error::IoError(e.to_string()))
                            .and_then(|child| act.insert_child(&session_id, child))
                            .and_then(|child_id| {
                                v.push(child_id);
                                Ok(fut::ok(v))
                            }).or_else(|e| {
                                vc.push(e.to_string());
                                Ok(fut::err(vc))
                            }).map_err(|e: Error| e)
                            .unwrap()
                    }));
                }
                Command::Stop { child_id } => {
                    future_chain = Box::new(future_chain.and_then(move |mut v, act, _ctx| {
                        let mut vc = v.clone();
                        info!("killing: {:?}", &child_id);
                        match act.get_session_mut(&session_id) {
                            Ok(session) => match session.processes.remove(&child_id) {
                                Some(child) => fut::Either::A(
                                    fut::wrap_future(
                                        SyncExecManager::from_registry().send(Exec::Kill(child)),
                                    ).map_err(|e, _act: &mut Self, _ctx| {
                                        vc.push(format!("{}", e));
                                        vc
                                    }).and_then(
                                        move |result, act, _ctx| {
                                            if let Ok(ExecResult::Kill(output)) = result {
                                                match act.get_session_mut(&session_id) {
                                                    Ok(mut session) => {
                                                        if session.processes.is_empty() {
                                                            session.status =
                                                                PeerSessionStatus::CONFIGURED;
                                                        };
                                                        v.push(output);
                                                        fut::ok(v)
                                                    }
                                                    Err(e) => {
                                                        v.push(e.to_string());
                                                        fut::err(v)
                                                    }
                                                }
                                            } else {
                                                v.push(format!("wrong result {:?}", result));
                                                fut::err(v)
                                            }
                                        },
                                    ),
                                ),
                                None => {
                                    v.push(Error::NoSuchChild(child_id).to_string());
                                    fut::Either::B(fut::err(v))
                                }
                            },
                            Err(e) => {
                                v.push(e.to_string());
                                fut::Either::B(fut::err(v))
                            }
                        }
                    }));
                }
                Command::AddTags(mut tags) => {
                    future_chain = Box::new(future_chain.and_then(move |mut v, act, _ctx| {
                        match act.get_session_mut(&session_id) {
                            Ok(session) => {
                                tags.into_iter().for_each(|tag| {
                                    session.tags.insert(tag);
                                });
                                v.push(format!(
                                    "tags inserted. Current tags are: {:?}",
                                    &session.tags
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
                        match act.get_session_mut(&session_id) {
                            Ok(session) => {
                                session.tags.retain(|t| !tags.contains(t));
                                v.push(format!(
                                    "tags removed. Current tags are: {:?}",
                                    &session.tags
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
                cmd => {
                    return ActorResponse::reply(Err(vec![format!(
                        "command {:?} unsupported",
                        cmd
                    )]));
                }
            }
        }
        ActorResponse::async(future_chain)
    }
}

// TODO: implement child process polling and status reporting
#[derive(Serialize, Deserialize, Debug)]
struct SessionStatus {
    session_id: String,
}

impl Handler<GetSessions> for HdMan {
    type Result = result::Result<Vec<PeerSessionInfo>, ()>;

    fn handle(&mut self, _msg: GetSessions, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self
            .sessions
            .iter()
            .map(|(id, session)| PeerSessionInfo {
                id: id.clone(),
                name: session.name.clone(),
                status: session.status.clone(),
                tags: Vec::from_iter(session.tags.clone().into_iter()),
                note: session.note.clone(),
                processes: session.processes.keys().cloned().collect(),
            }).collect())
    }
}

impl Handler<DestroySession> for HdMan {
    type Result = ActorResponse<HdMan, String, Error>;

    fn handle(
        &mut self,
        msg: DestroySession,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<DestroySession>>::Result {
        ActorResponse::async(match self.destroy_session(&msg.session_id) {
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
        let mut num_proc = 0;
        for session in self.sessions.values() {
            debug!("session status = {:?}", session.status);
            num_proc += session.processes.len();
        }
        debug!("result = {}", num_proc);
        MessageResult(match num_proc {
            0 => status::EnvStatus::Ready,
            _ => status::EnvStatus::Working,
        })
    }
}
