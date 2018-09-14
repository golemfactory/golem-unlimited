use actix::prelude::*;
use actix_web::HttpMessage;
use futures::future::Future;
use gu_actix::prelude::*;
use gu_p2p::rpc::*;
use gu_persist::config::ConfigModule;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{io, process, time};
use uuid::Uuid;

pub struct SessionInfo {
    id: String,
    image: Image,
    name: String,
    status: State,
    dirty: bool,
    tags: Vec<String>,
    note: Option<String>,
    children: HashMap<String, process::Child>,
}

pub enum State {
    PENDING, // during session creation
    CREATED, // after session creation, czysta
    RUNNING, // with at least one active child
    DIRTY,   // when no child is running, but some commands were already executed
    DESTROYING,
}

/// Host direct manager
pub struct HdMan {
    sessions: HashMap<String, SessionInfo>,
    image_cache_dir: PathBuf,
    sessions_dir: PathBuf,
    work_dir: PathBuf,
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
        ctx.bind::<Update>(Update::ID);
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
        ctx: &mut Self::Context,
    ) -> <Self as Handler<CreateSession>>::Result {
        let mut sess_id = Uuid::new_v4().to_string();
        while self.sessions.contains_key(&sess_id) {
            sess_id = Uuid::new_v4().to_string();
        }
        println!("newly created session_id={}", sess_id);
        self.sessions.insert(
            sess_id.clone(),
            SessionInfo {
                id: sess_id.clone(),
                image: msg.image.clone(),
                name: msg.name,
                status: State::PENDING,
                dirty: false,
                tags: msg.tags,
                note: msg.note,
                children: HashMap::new(),
            },
        );

        println!("hey! I'm downloading from: {:?}", msg.image);
        // TODO: download
        // TODO: untgz

        Ok(sess_id)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Update {
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

impl Update {
    const ID: u32 = 38;
}

impl Message for Update {
    // TODO: use error_chain
    type Result = Result<HashMap<String, String>, String>;
}

impl Handler<Update> for HdMan {
    type Result = Result<HashMap<String, String>, String>;

    fn handle(
        &mut self,
        msg: Update,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<Update>>::Result {
        let session = match self.sessions.get_mut(&msg.session_id) {
            Some(session) => session,
            None => return Err(format!("session_id {} not found", &msg.session_id)),
        };
        let mut cmd_outputs = HashMap::new();
        for cmd in msg.commands {
            match cmd {
                Command::Start { executable, args } => {
                    info!("executing: {} {:?}", executable, args);
                    match process::Command::new(&executable).args(&args).output() {
                        Ok(output) => {
                            if output.status.success() {
                                println!(
                                    "stdout: |{}|\nstderr: |{}|",
                                    String::from_utf8_lossy(&output.stdout),
                                    String::from_utf8_lossy(&output.stderr)
                                );
                                cmd_outputs.insert(
                                    format!("Start({}, {:?})", executable, args),
                                    String::from_utf8(output.stdout).unwrap_or("".into()),
                                );
                            }
                        }
                        Err(e) => return Err(format!("{:?}", e)),
                    }
                    session.dirty = true;
                }
                Command::StartAsync { executable, args } => {
                    info!("executing async: {} {:?}", executable, args);
                    let child_id = Uuid::new_v4().to_string();
                    match process::Command::new(&executable).args(&args).spawn() {
                        Ok(child) => {
                            session.children.insert(child_id.clone(), child);
                            session.dirty = true;
                            session.status = State::RUNNING;
                            cmd_outputs
                                .insert(format!("Start({}, {:?})", executable, args), child_id);
                        }
                        Err(e) => return Err(format!("{:?}", e)),
                    };
                }
                _ => {
                    cmd_outputs.insert(format!("{:?}", cmd), "unsupported".into());
                    ()
                }
            }
        }
        println!("{:?}", cmd_outputs);
        Ok(cmd_outputs)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct SessionStatus {
    session_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Status {}

impl Status {
    const ID: u32 = 38;
}

impl Message for Status {
    type Result = Result<HashMap<String, String>, String>;
}

#[derive(Serialize, Deserialize)]
struct Destroy {
    session_id: String, // uuid
}

//{"image": {"Url": "https://github.com/tworec/xmr-stak/releases/download/2.4.7-binaries/xmr-stak-MacOS.tgz"},
//"name": "monero mining",
//"tags": [],
//"note": "None"}

// "executable":"/Users/tworec/git/xmr-stak/bin/xmr-stak",
// "args": ["--noAMD", "--poolconf", "/Users/tworec/git/xmr-stak/pools.txt", "--httpd", "0"],

//{"session_id" : "214", "commands": [
//{"Start":{ "executable": "/bin/pwd", "args": [] } },
//{"Start":{ "executable": "/bin/ls", "args": ["-o"] } },
//{"Start":{ "executable": "/bin/date", "args": [] } }
//] }
