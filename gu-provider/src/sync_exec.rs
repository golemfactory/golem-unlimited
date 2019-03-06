use actix::{fut, prelude::*};
use error_chain::*;
use gu_actix::*;
use log::{debug, error};
use std::{io, path::PathBuf, process};

/// Synchronous executor
pub struct SyncExec;

impl Actor for SyncExec {
    type Context = SyncContext<Self>;
}

/// System service that manages synchronous executor instances
#[derive(Default)]
pub struct SyncExecManager {
    executor: Option<Addr<SyncExec>>,
}

impl Actor for SyncExecManager {
    type Context = Context<Self>;
}

impl Supervised for SyncExecManager {}

impl SystemService for SyncExecManager {}

impl SyncExecManager {
    fn executor(&mut self) -> &Addr<SyncExec> {
        let executor = match self.executor.take() {
            Some(v) => v,
            None => SyncArbiter::start(1, || SyncExec),
        };
        self.executor = Some(executor);
        self.executor.as_ref().unwrap()
    }
}

/// Message for executing commands
#[derive(Debug)]
pub enum Exec {
    Run {
        executable: String,
        args: Vec<String>,
        cwd: PathBuf,
    },
    Kill(process::Child),
}

#[derive(Debug)]
pub enum ExecResult {
    Run(process::Output),
    Kill(String),
}

impl Message for Exec {
    type Result = Result<ExecResult>;
}

impl Handler<Exec> for SyncExecManager {
    type Result = ActorResponse<SyncExecManager, ExecResult, Error>;

    fn handle(&mut self, msg: Exec, _ctx: &mut Self::Context) -> Self::Result {
        debug!("handling {:?}", &msg);
        ActorResponse::r#async(
            self.executor()
                .send(msg)
                .flatten_fut()
                .into_actor(self)
                .and_then(|res, _act, _ctx| fut::ok(res)),
        )
    }
}

impl Handler<Exec> for SyncExec {
    type Result = Result<ExecResult>;

    fn handle(&mut self, msg: Exec, _ctx: &mut Self::Context) -> Self::Result {
        debug!("synchronously executing: {:?}", &msg);
        match msg {
            Exec::Run {
                executable,
                args,
                cwd,
            } => {
                // TODO: critical section
                // TODO: env::set_current_dir(&base_dir)?;
                let output = process::Command::new(&executable)
                    .current_dir(&cwd)
                    .args(&args)
                    .output();
                match output {
                    Ok(output) => {
                        if output.status.success() {
                            debug!(
                                "stdout:\n{}\nstderr:\n{}\n",
                                String::from_utf8_lossy(&output.stdout),
                                String::from_utf8_lossy(&output.stderr)
                            );
                            Ok(ExecResult::Run(output))
                        } else {
                            Err(ErrorKind::ExecutionError(executable, args, output).into())
                        }
                    }
                    Err(e) => Err(e.into()),
                }
            }
            Exec::Kill(mut child) => child
                .kill()
                .and_then(|_| child.wait().map_err(From::from))
                .and_then(|_| Ok(ExecResult::Kill("Killed".into())))
                .map_err(From::from),
        }
    }
}

error_chain!(
    foreign_links {
        IoError(io::Error);
    }

    errors {
        MailboxError(e : MailboxError){}
        ExecutionError(exec: String, args: Vec<String>, output: process::Output) {
             display("failed to execute command: {}, {:?}, {:?}", exec, args, output)
        }
    }
);

impl From<MailboxError> for Error {
    fn from(e: MailboxError) -> Self {
        ErrorKind::MailboxError(e).into()
    }
}

#[cfg(test)]
mod test {
    use super::{Exec, ExecResult, SyncExecManager};
    use actix::prelude::*;
    use futures::Future;
    use gu_actix::flatten::FlattenFuture;

    #[test]
    fn test_sync_exec_fail() {
        System::run(|| {
            Arbiter::spawn(
                SyncExecManager::from_registry()
                    .send(Exec::Run {
                        executable: "/bin/ls".into(),
                        args: vec!["/1234567890asdfghjkl".into()],
                        cwd: "/".into()
                    }).flatten_fut()
                    .and_then(|o: ExecResult| match o {
                        ExecResult::Run(o) => {
                            assert!(!o.status.success());
                            assert_eq!(o.status.code(), Some(2));
                            assert_eq!(String::from_utf8_lossy(&o.stdout), "");
                            assert_eq!(String::from_utf8_lossy(&o.stderr), "ls: cannot access '/1234567890asdfghjkl': No such file or directory\n");
                            Ok(())
                        }
                        r => panic!("wrong result: {:?}", r),
                    }).map_err(|_| ())
                    .then(|_| Ok(System::current().stop())),
            )
        });
    }

    #[test]
    fn test_sync_exec_echo() {
        System::run(|| {
            Arbiter::spawn(
                SyncExecManager::from_registry()
                    .send(Exec::Run {
                        executable: "/bin/echo".into(),
                        args: vec!["zima".into()],
                        cwd: "/".into(),
                    })
                    .flatten_fut()
                    .and_then(|o: ExecResult| match o {
                        ExecResult::Run(o) => {
                            assert!(o.status.success());
                            assert_eq!(o.status.code(), Some(0));
                            assert_eq!(String::from_utf8_lossy(&o.stdout), "zima\n");
                            assert_eq!(String::from_utf8_lossy(&o.stderr), "");
                            Ok(())
                        }
                        r => panic!("wrong result: {:?}", r),
                    })
                    .map_err(|e| panic!("error: {}", e))
                    .then(|_| Ok(System::current().stop())),
            )
        });
    }

    #[test]
    #[ignore]
    fn test_sync_exec_pwd() {
        System::run(|| {
            Arbiter::spawn(
                SyncExecManager::from_registry()
                    .send(Exec::Run {
                        executable: "/bin/pwd".into(),
                        args: vec![],
                        cwd: "/var/tmp".into(),
                    })
                    .flatten_fut()
                    .and_then(|o: ExecResult| match o {
                        ExecResult::Run(o) => {
                            assert!(o.status.success());
                            assert_eq!(o.status.code(), Some(0));
                            // TODO: does not work on macos
                            assert_eq!(String::from_utf8_lossy(&o.stdout), "/var/tmp\n");
                            assert_eq!(String::from_utf8_lossy(&o.stderr), "");
                            Ok(())
                        }
                        r => panic!("wrong result: {:?}", r),
                    })
                    .map_err(|e| panic!("error: {}", e))
                    .then(|_| Ok(System::current().stop())),
            )
        });
    }
}
