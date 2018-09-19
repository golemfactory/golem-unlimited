use actix::fut;
use actix::prelude::*;
use gu_actix::*;
use std::{io, process};

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
    },
    Kill {
        child: process::Child,
    },
}

impl Message for Exec {
    type Result = Result<String>;
}

impl Handler<Exec> for SyncExecManager {
    type Result = ActorResponse<SyncExecManager, String, Error>;

    fn handle(&mut self, msg: Exec, _ctx: &mut Self::Context) -> Self::Result {
        debug!("handling {:?}", &msg);
        ActorResponse::async(
            self.executor()
                .send(msg)
                .flatten_fut()
                .into_actor(self)
                .and_then(|res, _act, _ctx| fut::ok(res)),
        )
    }
}

impl Handler<Exec> for SyncExec {
    type Result = Result<String>;

    fn handle(&mut self, msg: Exec, _ctx: &mut Self::Context) -> Self::Result {
        debug!("synchronously executing: {:?}", &msg);
        match msg {
            Exec::Run { executable, args } => {
                match process::Command::new(&executable).args(&args).output() {
                    Ok(output) => {
                        if output.status.success() {
                            Ok(format!(
                                "stdout:\n{}\nstderr:\n{}\n",
                                String::from_utf8_lossy(&output.stdout),
                                String::from_utf8_lossy(&output.stderr)
                            ))
                        } else {
                            Err(ErrorKind::ExecutionError(executable, args).into())
                        }
                    }
                    Err(e) => Err(e.into()),
                }
            }
            Exec::Kill { mut child } => child
                .kill()
                .and_then(|_| child.wait().map_err(From::from))
                .and_then(|_| Ok("Killed".into()))
                .map_err(From::from),
        }
    }
}

error_chain!(
    foreign_links {
        Io(io::Error);
    }

    errors {
        MailboxError(e : MailboxError){}
        ExecutionError(exec: String, args: Vec<String>) {
            display("failed to execute command: {}, {:?}", exec, args)
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
    use super::{Exec, SyncExecManager};
    use actix::prelude::*;
    use futures::Future;
    use gu_actix::flatten::FlattenFuture;

    #[test]
    fn test_sync_exec_date() {
        System::run(|| {
            Arbiter::spawn(
                SyncExecManager::from_registry()
                    .send(Exec {
                        executable: "/bin/echo".into(),
                        args: vec!["zima".into()],
                    }).flatten_fut()
                    .and_then(|o| {
                        assert!(o.status.success());
                        assert_eq!(o.status.code(), Some(0));
                        assert_eq!(String::from_utf8_lossy(&o.stdout), "zima\n");
                        assert_eq!(String::from_utf8_lossy(&o.stderr), "");
                        Ok(())
                    }).map_err(|e| panic!("error: {}", e))
                    .then(|_| Ok(System::current().stop())),
            )
        });
    }
}
