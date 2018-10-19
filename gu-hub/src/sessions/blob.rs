#![allow(proc_macro_derive_resolution_fallback)]

use super::responses::*;
use actix_web::http::header::HeaderValue;
use actix_web::{dev::Payload, fs::NamedFile};
use futures::Future;
use gu_base::files::{read_async, write_async};
use std::{fs, fs::File, io, path::PathBuf};
use sha1::Sha1;
use futures::Stream;
use futures::future;
use futures::future::Shared;
use std::collections::BTreeMap;
use futures::sync::oneshot::{Sender, self};
use actix::Recipient;
use actix::Actor;
use actix::Context;
use actix::Handler;
use actix::ActorResponse;
use actix::AsyncContext;
use futures::future::SharedItem;
use std::ops::Deref;
use futures::future::SharedError;
use actix::WrapFuture;
use actix::Addr;

struct FileLockActor {
    to_notify: Vec<Sender<()>>,
    readers: usize,
    writers: usize,

    path: PathBuf,

    /// Future that gives current sha1 checksum of the file
    sha1_fut: Shared<Box<Future<Item=Sha1, Error=SessionErr> + Send>>,
    /// Map of currently running, outer futures
    write_futs: BTreeMap<(u64, u64), Shared<Box<Future<Item=(), Error=()> + Send>>>
}


impl FileLockActor {
    fn new(path: PathBuf) -> Self {
        FileLockActor {
            path,
            ..Default::default()
        }
    }
}

impl Default for FileLockActor {
    fn default() -> Self {
        let x: Box<Future<Item=Sha1, Error=SessionErr> + Send> = Box::new(future::ok(Sha1::new()));

        FileLockActor {
            to_notify: Vec::new(),
            readers: 0,
            writers: 0,
            path: PathBuf::default(),
            sha1_fut: x.shared(),
            write_futs: BTreeMap::new(),
        }
    }
}

impl Actor for FileLockActor {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<ReadAccess, SessionErr>")]
struct ReadAccessRequest;

impl Handler<ReadAccessRequest> for FileLockActor {
    type Result = ActorResponse<Self, ReadAccess, SessionErr>;

    fn handle(&mut self, _msg: ReadAccessRequest, ctx: &mut Context<Self>) -> Self::Result {
        ActorResponse::async(match self.writers {
            0 => future::Either::A({
                let rec = ctx.address().recipient();
                self.readers += 1;

                self.sha1_fut.clone()
                    .and_then(move |sha: SharedItem<Sha1>| {
                        Ok(ReadAccess {
                            sha1: sha.deref().clone(),
                            actor: rec,
                        })
                    })
                    .map_err(|err: SharedError<SessionErr>| err.deref().clone())
                    .map_err(|err: SessionErr| err)
            }),
            _ => future::Either::B(future::err(SessionErr::BlobLockedError)),
        }.into_actor(self))
    }
}

#[derive(Message)]
#[rtype(result = "Result<WriteAccess, SessionErr>")]
struct WriteAccessRequest;

impl Handler<WriteAccessRequest> for FileLockActor {
    type Result = ActorResponse<Self, WriteAccess, SessionErr>;

    fn handle(&mut self, _msg: WriteAccessRequest, ctx: &mut Context<Self>) -> Self::Result {
        self.writers += 1;
        let x: Box<Future<Item=Sha1, Error=SessionErr> + Send> = Box::new(recalculate_sha1(self.path.clone()));
        self.sha1_fut = x.shared();
        let readers = self.readers;
        let recipient = ctx.address().recipient();
        let dag_fut = write_dag_future(&mut self.write_futs);

        let rec = match readers {
            0 => future::Either::A(future::ok(())),
            _ => future::Either::B({
                let (send, rec) = oneshot::channel();
                self.to_notify.push(send);
                rec
            }),
        };

        ActorResponse::async(
            dag_fut
                .and_then(|_| rec.map_err(|e| SessionErr::FileError(e.to_string())))
                .and_then(|_| Ok(WriteAccess {
                    actor: recipient,
                })).into_actor(self)
        )
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct DropReader;

struct ReadAccess {
    sha1: Sha1,
    actor: Recipient<DropReader>,
}

impl Drop for ReadAccess {
    fn drop(&mut self) {
        let _ = self.actor.do_send(DropReader);
    }
}

impl Handler<DropReader> for FileLockActor {
    type Result = ();

    fn handle(&mut self, _msg: DropReader, _ctx: &mut Context<Self>) -> () {
        self.readers -= 1;
        if self.readers == 0 {
            let vec = self.to_notify.drain(..);
            for writer in vec.into_iter() {
                let _ = writer.send(());
            }
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct DropWriter;

struct WriteAccess {
    actor: Recipient<DropWriter>,
}

impl Drop for WriteAccess {
    fn drop(&mut self) {
        let _ = self.actor.do_send(DropWriter);
    }
}

impl Handler<DropWriter> for FileLockActor {
    type Result = ();

    fn handle(&mut self, _msg: DropWriter, _ctx: &mut Context<Self>) -> () {
        self.writers -= 1;
    }
}

fn recalculate_sha1(path: PathBuf) -> impl Future<Item = Sha1, Error = SessionErr> {
    read_async(path.clone()).fold(Sha1::new(), |mut sha, chunk| {
        sha.update(chunk.as_ref());
        Ok(sha).map_err(|()| "")
    })
        .map_err(|e| SessionErr::FileError(e))
        .and_then(|sha| Ok(sha))
}

// Generate future that will complete when it will be possible to write to file
fn write_dag_future(free: &mut BTreeMap<(u64, u64), Shared<Box<Future<Item=(), Error=()> + Send>>>) -> impl Future<Item=(), Error=SessionErr> + Send {
    use std::u64::{MAX, MIN};

    let write_begin = MIN;
    let write_end = MAX;
    let mut wait_list = Vec::new();
    let mut remove_list = Vec::new();

    for elt in free.range(..(write_begin, write_begin)).rev() {
        let ((begin, end), shared) = elt;

        if *end > write_begin {
            if shared.peek().is_some() {
                remove_list.push((*begin, *end))
            } else {
                wait_list.push(shared.clone())
            }
        } else {
            break
        }
    }

    for elt in free.range((write_begin, write_begin)..) {
        let ((begin, end), shared) = elt;

        if *begin < write_end {
            if shared.peek().is_some() {
                remove_list.push((*begin, *end))
            } else {
                wait_list.push(shared.clone())
            }

            // ranges contained in new range
            if *end <= write_end {
                remove_list.push((*begin, *end))
            }
        } else {
            break
        }
    }

    for x in remove_list {
        free.remove(&x);
    }

    //wait_list.insert((min, max), )

    let x: Box<Future<Item=(), Error=()> + Send> = Box::new(future::join_all(wait_list).then(|_| Ok(())));
    let x: Shared<Box<_>> = x.shared();
    free.insert((write_begin, write_end), x.clone());
    x.and_then(|_| Ok(())).map_err(|_| SessionErr::FileError("Lock on writer???!!!".to_string()))
}

#[derive(Clone)]
pub struct Blob {
//    /// Path to blob file
//    path: PathBuf,
//    /// Indicates if any file was sent on blob
//    sent: bool,
//    /// Calculated sha1 checksum on file
//    sha1: Option<HeaderValue>,
//    //lock: FileLockActor,

    path: PathBuf,
    lock: Addr<FileLockActor>,
}

impl Blob {
    pub fn new(path: PathBuf) -> io::Result<Blob> {
        File::create(&path)?;

        Ok(Blob {
            path: path.clone(),
            lock: FileLockActor::new(path).start(),
        })
    }

    pub fn from_existing(path: PathBuf) -> Blob {
        Blob {
            path: path.clone(),
            lock: FileLockActor::new(path).start(),
        }
    }

    pub fn write(self, fut: Payload) -> impl Future<Item = SessionOk, Error = SessionErr> {
        self.lock.send(WriteAccessRequest)
            .map_err(|e| SessionErr::MailboxError(e.to_string()))
            .and_then(|a| a)
            .and_then(move |_access: WriteAccess| {
                write_async(fut, self.path.clone())
                    .map_err(|e| SessionErr::FileError(e))
            })
            .and_then(|_a| Ok(SessionOk::Ok))
    }

    pub fn read(self) -> impl Future <Item=(NamedFile, HeaderValue), Error=SessionErr> {
        self.lock.send(ReadAccessRequest)
            .map_err(|e| SessionErr::MailboxError(e.to_string()))
            .and_then(|a| a)
            .and_then(move |access: ReadAccess| {
                NamedFile::open(&self.path)
                    .map_err(|e| SessionErr::FileError(e.to_string()))
                    .map(|f| (f, HeaderValue::from_str(&access.sha1.digest().to_string()).unwrap()))
            })
    }

    pub fn clean_file(&self) -> io::Result<()> {
        match (&self.path).exists() {
            true => fs::remove_file(&self.path),
            false => Ok(()),
        }
    }
}
