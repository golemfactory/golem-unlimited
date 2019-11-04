use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::string;

use actix::prelude::*;
use futures::unsync::oneshot;
use futures::{future, prelude::*};
use tokio_io::io;
use tokio_process::{Child, CommandExt};

use gu_actix::{async_result, async_try};
use gu_model::envman;

type Map<K, V> = HashMap<K, V>;

#[derive(Debug, Hash, PartialOrd, PartialEq, Eq, Clone, Copy)]
pub struct Pid(u32);

impl ToString for Pid {
    fn to_string(&self) -> String {
        ToString::to_string(&self.0)
    }
}

impl From<u32> for Pid {
    fn from(pid: u32) -> Self {
        Pid(pid)
    }
}

impl FromStr for Pid {
    type Err = <u32 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        Ok(Pid(s.parse()?))
    }
}

fn io_err<E: std::error::Error + Send + Sync + 'static>(e: E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e)
}

#[derive(Default)]
pub struct ProcessPool {
    // process pool workdir
    work_dir: PathBuf,
    white_list: HashSet<PathBuf>,
    main_process: Option<Pid>,
    exec_processes: Map<Pid, oneshot::Sender<()>>,
}

impl ProcessPool {
    pub fn with_work_dir<P: Into<PathBuf>>(work_dir: P) -> Self {
        ProcessPool {
            white_list: Default::default(),
            work_dir: work_dir.into(),
            main_process: None,
            exec_processes: Map::new(),
        }
    }

    pub fn with_exec(mut self, path: impl Into<PathBuf>) -> Self {
        self.white_list.insert(path.into());
        self
    }
}

impl Actor for ProcessPool {
    type Context = Context<Self>;
}

impl ProcessPool {
    fn exec<P: AsRef<Path>, S: AsRef<OsStr>, I: IntoIterator<Item = S>>(
        &mut self,
        ctx: &mut <Self as Actor>::Context,
        executable: &P,
        args: I,
    ) -> impl Future<Item = (String, String), Error = String> {
        let exec = executable.as_ref();
        if !self.white_list.contains(exec) {
            if exec.is_absolute() {
                return async_try!(Err(format!("invalid executable {:?}", exec)));
            }
        }

        let exec_path = self.work_dir.join(exec);
        eprintln!("running = {:?}", &exec_path);
        let mut child: Child = async_try!(Command::new(exec_path)
            .args(args)
            .current_dir(&self.work_dir)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn_async()
            .map_err(|e| format!("run: {}", e)));
        let stdout = child.stdout().take().unwrap();
        let stderr = child.stderr().take().unwrap();

        self.spawn_child(ctx, child);

        async_result!(io::read_to_end(stdout, Vec::new())
            .map_err(|e| format!("stdout read fail: {}", e))
            .and_then(|(_stdout, bytes)| string::String::from_utf8(bytes)
                .map_err(|e| format!("invalid output: {}", e)))
            .join(
                io::read_to_end(stderr, Vec::new())
                    .map_err(|e| format!("stderr read fail: {}", e))
                    .and_then(|(_stdout, bytes)| string::String::from_utf8(bytes)
                        .map_err(|e| format!("invalid output: {}", e)))
            ))
    }

    fn stop_process(&mut self, pid: Pid) -> Result<(), String> {
        if let Some(tx) = self.exec_processes.remove(&pid) {
            tx.send(()).map_err(|_e| format!("kill"))?
        }
        Ok(())
    }

    fn kill_all(&mut self) {
        // TODO: log error
        self.exec_processes.drain().for_each(|(_pid, tx)| {
            let _ = tx.send(());
        });
    }

    fn spawn_child(&mut self, ctx: &mut <Self as Actor>::Context, child: Child) -> Pid {
        let pid = Pid::from(child.id());

        let (tx, rx) = oneshot::channel();

        self.exec_processes.insert(pid, tx);

        ctx.spawn(
            child
                .select2(rx)
                .then(|r| match r {
                    Ok(future::Either::A((exit_status, _rx))) => {
                        future::Either::A(future::ok(exit_status))
                    }
                    Ok(future::Either::B(((), mut child))) => match child.kill() {
                        Err(e) => future::Either::A(future::err(e)),
                        Ok(()) => future::Either::B(child),
                    },
                    Err(future::Either::A((e, _rx))) => future::Either::A(future::err(e)),
                    Err(future::Either::B((e, _child))) => {
                        future::Either::A(future::err(io_err(e)))
                    }
                })
                .into_actor(self)
                .then(move |_r, act, _ctx| {
                    act.exec_processes.remove(&pid);
                    if Some(pid) == act.main_process {
                        act.kill_all()
                    }
                    fut::ok(())
                }),
        );

        pid
    }
}

pub struct Exec {
    pub executable: PathBuf,
    pub args: Vec<String>,
}

impl Message for Exec {
    type Result = Result<(String, String), String>;
}

impl Handler<Exec> for ProcessPool {
    type Result = ActorResponse<ProcessPool, (String, String), String>;

    fn handle(&mut self, msg: Exec, ctx: &mut Self::Context) -> <Self as Handler<Exec>>::Result {
        ActorResponse::r#async(self.exec(ctx, &msg.executable, msg.args).into_actor(self))
    }
}

pub struct List;

impl Message for List {
    type Result = Vec<Pid>;
}

impl Handler<List> for ProcessPool {
    type Result = MessageResult<List>;

    fn handle(&mut self, _msg: List, _ctx: &mut Self::Context) -> <Self as Handler<List>>::Result {
        MessageResult(self.exec_processes.keys().cloned().collect())
    }
}

pub struct Stop(pub Pid);

impl Message for Stop {
    type Result = Result<(), String>;
}

impl Handler<Stop> for ProcessPool {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: Stop, _ctx: &mut Self::Context) -> <Self as Handler<Stop>>::Result {
        self.stop_process(msg.0)
    }
}

pub struct KillAll;

impl Message for KillAll {
    type Result = Result<(), envman::Error>;
}

impl Handler<KillAll> for ProcessPool {
    type Result = Result<(), envman::Error>;

    fn handle(&mut self, msg: KillAll, ctx: &mut Self::Context) -> Self::Result {
        Ok(self.kill_all())
    }
}
