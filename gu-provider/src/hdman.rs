use actix::fut;
use actix::prelude::*;
use actix_web::HttpMessage;
//use futures::future;
use futures::prelude::*;
use gu_actix::prelude::*;
use gu_p2p::rpc::*;
use gu_persist::config::ConfigModule;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
//use std::sync::Arc;
use super::sync_exec::{Exec, SyncExecManager};
use gu_p2p::rpc::peer::{PeerSessionInfo, PeerSessionStatus};
use std::{io, process, time};
use uuid::Uuid;

pub struct SessionInfo {
    image: Image,
    name: String,
    status: PeerSessionStatus,
    dirty: bool,
    tags: Vec<String>,
    note: Option<String>,
    processes: HashMap<String, process::Child>,
}

/// Host direct manager
pub struct HdMan {
    sessions: HashMap<String, SessionInfo>,
    image_cache_dir: PathBuf,
    sessions_dir: PathBuf,
    work_dir: PathBuf,
}

impl HdMan {
    fn scan_for_processes(&mut self) {
        for sess_info in self.sessions.values_mut().into_iter() {
            let finished: Vec<String> = sess_info
                .processes
                .iter_mut()
                .filter_map(|p| match p.1.try_wait() {
                    Ok(Some(_exit_st)) => Some(p.0.clone()),
                    _ => None,
                })
                .collect();
            for f in finished {
                sess_info.processes.remove(&f);
                info!("finished {:?}; removing", f)
            }
        }
    }

    fn get_session_mut(&mut self, session_id: &String) -> Result<&mut SessionInfo, String> {
        match self.sessions.get_mut(session_id) {
            Some(session) => Ok(session),
            None => Err(format!("session_id {} not found", &session_id)),
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

impl Message for CreateSession {
    type Result = Result<String, ()>; // sess_id --> uuid
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

pub fn untgz<P: AsRef<Path>>(input_path: P, output_path: P) -> Result<(), io::Error> {
    use flate2::read::GzDecoder;
    use std::fs;
    use tar::Archive;

    let d = GzDecoder::new(fs::File::open(input_path)?);
    let mut ar = Archive::new(d);
    ar.unpack(output_path)
}

impl Handler<CreateSession> for HdMan {
    type Result = Result<String, ()>;

    fn handle(
        &mut self,
        msg: CreateSession,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<CreateSession>>::Result {
        let mut sess_id = Uuid::new_v4().to_string();
        while self.sessions.contains_key(&sess_id) {
            sess_id = Uuid::new_v4().to_string();
        }
        debug!("newly created session_id={}", sess_id);
        self.sessions.insert(
            sess_id.clone(),
            SessionInfo {
                image: msg.image.clone(),
                name: msg.name,
                status: PeerSessionStatus::PENDING,
                dirty: false,
                tags: msg.tags,
                note: msg.note,
                processes: HashMap::new(),
            },
        );

        debug!("hey! I'm downloading from: {:?}", msg.image);
        // TODO: download
        // TODO: untgz

        Ok(sess_id)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct SessionUpdate {
    session_id: String,
    commands: Vec<Command>,
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, Debug)]
enum Command {
    Start {
        // return cmd output
        executable: String,
        args: Vec<String>,
    },
    StartAsync {
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
    // TODO: use error_chain
    type Result = Result<Vec<String>, String>;
}

impl Handler<SessionUpdate> for HdMan {
    type Result = ActorResponse<HdMan, Vec<String>, String>; // TODO: Err -> (succeded cmds, first failed err msg)

    fn handle(
        &mut self,
        msg: SessionUpdate,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<SessionUpdate>>::Result {
        let mut future_chain: Box<
            ActorFuture<Item = Vec<String>, Error = String, Actor = Self>,
        > = Box::new(fut::ok(Vec::new()));

        for cmd in msg.commands {
            let session_id = msg.session_id.clone();

            match cmd {
                Command::Start { executable, args } => {
                    future_chain = Box::new(future_chain.and_then(|mut v, act, _ctx| {
                        SyncExecManager::from_registry()
                            .send(Exec { executable, args })
                            .flatten_fut()
                            .map_err(|e| format!("{}", e)) // TODO: chain err
                            .into_actor(act)
                            .and_then(move |output, act, _ctx| {
                                info!("sync cmd output: {:?}", output);
                                if output.status.success() {
                                    v.push(String::from_utf8(output.stdout).unwrap_or("".into()));
                                    let session = act.get_session_mut(&session_id).unwrap(); // TODO
                                    session.dirty = true;
                                    fut::ok(v)
                                } else {
                                    fut::err(format!("cmd failed: {:?}", output))
                                }
                            })
                    }));
                }
                Command::StartAsync { executable, args } => {
                    future_chain = Box::new(future_chain.and_then(move |mut v, act, _ctx| {
                        info!("executing async: {} {:?}", executable, args);
                        match process::Command::new(&executable).args(&args).spawn() {
                            Ok(child) => {
                                let session = act.get_session_mut(&session_id).unwrap(); // TODO
                                let child_id = Uuid::new_v4().to_string();
                                session.processes.insert(child_id.clone(), child);
                                session.dirty = true;
                                session.status = PeerSessionStatus::RUNNING;
                                v.push(child_id);
                                fut::ok(v)
                            }
                            Err(e) => fut::err(format!("{:?}", e)),
                        }
                    }));
                }
                Command::Stop { child_id } => {
                    info!("killing: {:?}", &child_id);
                    future_chain = Box::new(future_chain.and_then(move |mut v, act, _ctx| {
                        let session = act.get_session_mut(&session_id).unwrap(); // TODO
                        match session.processes.get_mut(&child_id) {
                            Some(child) => match child.kill() {
                                Ok(_) => {
                                    v.push("Killed".into());
                                    fut::ok(v)
                                }
                                Err(e) => fut::err(format!("{:?}", e)),
                            },
                            None => fut::err(format!("child {:?} not found", child_id)),
                        }
                    }));
                }
                cmd => {
                    future_chain = Box::new(fut::err(format!("{:?} unsupported", cmd)));
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
    type Result = Result<Vec<PeerSessionInfo>, ()>; // TODO: error chain
}

impl Handler<GetSessions> for HdMan {
    type Result = Result<Vec<PeerSessionInfo>, ()>;

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
            })
            .collect())
    }
}

#[derive(Serialize, Deserialize)]
struct SessionDestroy {
    session_id: String, // uuid
}

// CreateSession - 37
//
//{"image": {"Url": "https://github.com/tworec/xmr-stak/releases/download/2.4.7-binaries/xmr-stak-MacOS.tgz"},
//"name": "monero mining",
//"tags": [],
//"note": "None"}

// "executable":"/Users/tworec/git/xmr-stak/bin/xmr-stak",
// "args": ["--noAMD", "--poolconf", "/Users/tworec/git/xmr-stak/pools.txt", "--httpd", "0"],

// SessionUpdate - 38
//{"session_id" : "214", "commands": [
//{"Start":{ "executable": "/bin/pwd", "args": [] } },
//{"Start":{ "executable": "/bin/ls", "args": ["-o"] } },
//{"Start":{ "executable": "/bin/date", "args": [] } }
//] }
