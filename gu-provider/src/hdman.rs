use super::sync_exec::{Exec, ExecResult, SyncExecManager};
use actix::fut;
use actix::prelude::*;
use actix_web::HttpMessage;
use futures::prelude::*;
use gu_actix::prelude::*;
use gu_p2p::rpc::peer::{PeerSessionInfo, PeerSessionStatus};
use gu_p2p::rpc::*;
use gu_persist::config::ConfigModule;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{fmt, process, result, time};
use uuid::Uuid;

pub struct SessionInfo {
    image: Image,
    name: String,
    status: PeerSessionStatus,
    dirty: bool,
    // TODO: use it when last child dies
    tags: Vec<String>,
    note: Option<String>,
    processes: HashMap<String, process::Child>,
    work_dir: PathBuf,
}

impl SessionInfo {
    fn insert_process(&mut self, child: process::Child) -> String {
        let mut id = new_id();
        while self.processes.contains_key(&id) {
            id = new_id();
        }
        self.processes.insert(id.clone(), child);
        self.dirty = true;
        self.status = PeerSessionStatus::RUNNING;
        id
    }
}

/// Host direct manager
pub struct HdMan {
    sessions: HashMap<String, SessionInfo>,
    image_cache_dir: PathBuf,
    sessions_dir: PathBuf,
    work_dir: PathBuf,
}

impl HdMan {
    fn generate_session_id(&self) -> String {
        let mut id = new_id();
        while self.sessions.contains_key(&id) {
            id = new_id();
        }
        id
    }

    fn get_session_mut(&mut self, session_id: &String) -> Result<&mut SessionInfo, Error> {
        match self.sessions.get_mut(session_id) {
            Some(session) => Ok(session),
            None => Err(Error::NoSuchSession(session_id.clone())),
        }
    }

    fn insert_child(&mut self, session_id: &String, child: process::Child) -> Result<String, Error> {
        Ok(self.get_session_mut(&session_id)?.insert_process(child))
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

pub fn start(config: &ConfigModule) -> Addr<HdMan> {
    start_actor(HdMan {
        sessions: HashMap::new(),
        image_cache_dir: config.cache_dir().to_path_buf().join("images"),
        sessions_dir: config.cache_dir().to_path_buf().join("sessions"),
        work_dir: config.work_dir().into(),
    })
}

impl Actor for HdMan {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.bind::<CreateSession>(CreateSession::ID);
        ctx.bind::<SessionUpdate>(SessionUpdate::ID);
        ctx.bind::<GetSessions>(GetSessions::ID);
        ctx.run_interval(time::Duration::from_secs(10), |act, _| {
            act.scan_for_processes()
        });
    }
}

/// Message for session creation: local provisioning: downloads and unpacks the binaries
#[derive(Serialize, Deserialize)]
struct CreateSession {
    image: Image,
    name: String,
    tags: Vec<String>,
    note: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Image {
    Url(String),
}

impl CreateSession {
    const ID: u32 = 37;
}

/// returns session_id
impl Message for CreateSession {
    type Result = result::Result<String, ()>;
}

pub fn download(url: &str, output_path: String) -> Box<Future<Item = (), Error = ()>> {
    use actix_web::client;
    use write_to::to_file;

    let client_request = client::ClientRequest::get(url).finish().unwrap();

    Box::new(
        client_request
            .send()
            .timeout(time::Duration::from_secs(300))
            .map_err(|e| error!("send download request: {}", e))
            .and_then(|resp| {
                to_file(resp.payload(), output_path)
                    .map_err(|e| error!("write to file error: {}", e))
            }),
    )
}

pub fn untgz<P: AsRef<Path>>(input_path: P, output_path: P) -> Result<(), Error> {
    use flate2::read::GzDecoder;
    use std::fs;
    use tar::Archive;

    let d = GzDecoder::new(fs::File::open(input_path).map_err(|e| Error::IoError(e.to_string()))?);
    let mut ar = Archive::new(d);
    ar.unpack(output_path).map_err(|e| Error::IoError(e.to_string()))
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

impl Handler<CreateSession> for HdMan {
    type Result = result::Result<String, ()>;

    fn handle(
        &mut self,
        msg: CreateSession,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<CreateSession>>::Result {
        let session_id = self.generate_session_id();

        let session = SessionInfo {
            image: msg.image.clone(),
            name: msg.name,
            status: PeerSessionStatus::PENDING,
            dirty: false,
            tags: msg.tags,
            note: msg.note,
            processes: HashMap::new(),
            work_dir: self.work_dir.join(&session_id),
        };

        debug!("newly created session id={}", session_id);
        self.sessions.insert(session_id.clone(), session);

        debug!("hey! I'm downloading from: {:?}", msg.image);
        // TODO: download
        // TODO: untgz

        self.sessions.get_mut(&session_id).unwrap().status = PeerSessionStatus::CREATED;
        Ok(session_id)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct SessionUpdate {
    session_id: String,
    commands: Vec<Command>,
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, Debug)]
enum Command {
    Exec {
        // return cmd output
        executable: String,
        args: Vec<String>,
    },
    Start {
        // return child process id
        executable: String,
        args: Vec<String>,
        // TODO: consider adding tags here
    },
    Stop {
        child_id: String,
    },
    AddTags(Vec<String>),
    DelTags(Vec<String>),
    DumpFile {
        data: Vec<u8>,
        file_name: String,
    },
}

impl SessionUpdate {
    const ID: u32 = 38;
}

impl Message for SessionUpdate {
    type Result = result::Result<Vec<String>, Vec<String>>;
}

impl Handler<SessionUpdate> for HdMan {
    /// ok: succeeded cmds output
    /// err: all succeeded cmds output till first failure, plus failed cmd err msg
    type Result = ActorResponse<HdMan, Vec<String>, Vec<String>>;

    fn handle(&mut self, msg: SessionUpdate, _ctx: &mut Self::Context) -> Self::Result {
        if !self.sessions.contains_key(&msg.session_id) {
            return ActorResponse::reply(Err(vec![format!(
                "session_id {} not found",
                &msg.session_id
            )]));
        }

        let mut future_chain: Box<
            ActorFuture<Item = Vec<String>, Error = Vec<String>, Actor = Self>,
        > = Box::new(fut::ok(Vec::new()));

        for cmd in msg.commands {
            let session_id = msg.session_id.clone();

            match cmd {
                Command::Exec { executable, args } => {
                    future_chain = Box::new(future_chain.and_then(move |mut v, act, _ctx| {
                        let mut vc = v.clone();
                        info!("executing sync: {} {:?}", executable, args);
                        SyncExecManager::from_registry()
                            .send(Exec::Run { executable, args })
                            .flatten_fut()
                            .map_err(|e| {
                                vc.push(e.to_string());
                                vc
                            })
                            .into_actor(act)
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
                    future_chain = Box::new(future_chain.and_then(move |mut v, act, _ctx| {
                        info!("executing async: {} {:?}", executable, args);
                        // TODO: critical section
                        // TODO: env::set_current_dir(&base_dir)?;
                        let mut vc = v.clone();
                        process::Command::new(&executable)
                            .args(&args)
                            .spawn()
                            .map_err(|e| Error::IoError(e.to_string()))
                            .and_then(|child| {
                                act.insert_child(&session_id, child)
                            }).and_then(|child_id| {
                                v.push(child_id);
                                Ok(fut::ok(v))
                            }).or_else(|e| {
                                vc.push(e.to_string());
                                Ok(fut::err(vc))
                            })
                            .map_err(|e: Error| e)
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
                                    v.push(format!("child {:?} not found", child_id));
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

#[derive(Serialize, Deserialize, Debug)]
struct SessionStatus {
    session_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct GetSessions {}

impl GetSessions {
    const ID: u32 = 39;
}

impl Message for GetSessions {
    type Result = result::Result<Vec<PeerSessionInfo>, ()>;
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
                tags: session.tags.clone(),
                note: session.note.clone(),
            }).collect())
    }
}

#[derive(Serialize, Deserialize)]
struct SessionDestroy {
    session_id: String,
}

//error_chain!(
//    foreign_links {
//        IoError(io::Error);
//    }
//
//    errors {
//        MailboxError(e : MailboxError){}
//        NoSuchSession(id: String) {
//            display("session {} not found", id)
//        }
//        NoSuchChild(id: String) {
//            display("child {} not found", id)
//        }
//    }
//);
//
//impl From<MailboxError> for Error {
//    fn from(e: MailboxError) -> Self {
//        ErrorKind::MailboxError(e).into()
//    }
//}

#[derive(Serialize, Deserialize, Debug)]
pub enum Error {
    IoError(String),
    NoSuchSession(String),
    NoSuchChild(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IoError(msg) => write!(f, "IoError: {}", msg)?,
            Error::NoSuchSession(msg) => write!(f, "NoSuchSession: {}", msg)?,
            Error::NoSuchChild(msg) => write!(f, "NoSuchChild: {}", msg)?,
        }
        Ok(())
    }
}