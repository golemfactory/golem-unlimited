use actix::prelude::*;
//use actix_web::client;
use futures::future::Future;
use gu_actix::prelude::*;
use gu_p2p::rpc::*;
use gu_persist::config::ConfigModule;
use gu_persist::config::{ConfigManager, GetConfig, HasSectionId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;

//use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    keystore: String,
}

impl HasSectionId for Config {
    const SECTION_ID: &'static str = "hdman";
}

impl Default for Config {
    fn default() -> Self {
        Config {
            keystore: "z".into(),
        }
    }
}

pub struct SessionInfo {
    id: String,
    image: Image,
    name: String,
    tags: Vec<String>,
    note: Option<String>,
}

/// Host direct manager
pub struct HdMan {
    sessions: HashMap<String, SessionInfo>,
    work_dir: PathBuf,
    cache_dir: PathBuf,
}

pub fn start(config: &ConfigModule) -> Addr<HdMan> {
    start_actor(HdMan {
        sessions: HashMap::new(),
        work_dir: config.work_dir().into(),
        cache_dir: config.cache_dir().into(),
    })
}

impl Actor for HdMan {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.bind::<CreateSession>(CreateSession::ID);
        ctx.bind::<Start>(Start::ID);
        ConfigManager::from_registry()
            .send(GetConfig::new())
            .flatten_fut()
            .and_then(|c: Arc<Config>| Ok(println!("have config:")))
            .map_err(|_| ())
            .into_actor(self)
            .wait(ctx);
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

#[derive(Serialize, Deserialize, Debug)]
enum Image {
    Url(String),
}

impl CreateSession {
    const ID: u32 = 37;
}

impl Message for CreateSession {
    type Result = Result<String, ()>; // sess_id --> uuid
}

impl Handler<CreateSession> for HdMan {
    type Result = Result<String, ()>;

    fn handle(
        &mut self,
        msg: CreateSession,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<CreateSession>>::Result {
        println!("hey! I'm downloading from: {:?}", msg.image);
        //        client::get("http://www.rust-lang.org").finish().unwrap() //TODO: use `?`
        //            .send()
        //            .map_err(|_| ())
        //            .and_then(|response| {                // <- server http response
        //                println!("Response: {:?}", response);
        //                Ok(())
        //            });

        //        let sess_id = Uuid::new_v4();
        //        println!("{}", sess_id);
        //
        //        Ok(sess_id)
        Err(())
    }
}

struct Update {
    session_id: String,
    commands: Vec<Command>,
}

enum Command {
    Start {
        executable: String,
        args: Vec<String>,
    },
    Stop,
    AddTags(Vec<String>),
    DelTags(Vec<String>),
    DumpFile {
        data: Vec<u8>,
        file_name: String,
    },
}

/// Message for session start - invokes supplied binary
#[derive(Serialize, Deserialize)]
struct Start {
    session_id: String, // uuid
    executable: String,
    args: Vec<String>,
}

impl Start {
    const ID: u32 = 38;
}

impl Message for Start {
    type Result = Result<String, ()>;
}

impl Handler<Start> for HdMan {
    type Result = Result<String, ()>;

    fn handle(&mut self, msg: Start, ctx: &mut Self::Context) -> <Self as Handler<Start>>::Result {
        println!("hey! I'm executing: {} {:?}", msg.executable, msg.args);
        let res = process::Command::new(msg.executable)
            .args(msg.args)
            .output();
        if let Ok(output) = res {
            if output.status.success() {
                println!(
                    "stdout: |{}|\nstderr: |{}|",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                return Ok(String::from_utf8(output.stdout).unwrap_or("".into()));
            }
        }
        Err(())
    }
}

#[derive(Serialize, Deserialize)]
struct Stop {
    session_id: String, // uuid
}

//{"session_id": "s",
// "executable":"/Users/tworec/git/xmr-stak/bin/xmr-stak",
// "args": ["--noAMD", "--poolconf", "/Users/tworec/git/xmr-stak/pools.txt", "--httpd", "0"],
//{"image": {"Url": "https://github.com/tworec/xmr-stak/releases/download/2.4.7-binaries/xmr-stak-MacOS.tgz"},
//"name": "monero mining",
//"tags": []}
