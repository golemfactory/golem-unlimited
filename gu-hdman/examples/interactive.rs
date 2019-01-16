use actix::prelude::*;
use futures::prelude::*;
use futures::sync::mpsc;
use gu_actix::flatten::FlattenFuture;
use gu_hdman::process_pool::*;
use std::io::BufRead;
use std::result::Result;
use std::{io, thread};

fn stdin() -> impl Stream<Item = String, Error = io::Error> {
    let (tx, rx) = mpsc::unbounded();

    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            tx.unbounded_send(line).unwrap()
        }
    });

    rx.map_err(|e| io::Error::new(io::ErrorKind::Other, "x"))
        .and_then(|r| r)
}

fn main() {
    let pp = ProcessPool::with_work_dir("/tmp").start();

    System::run(move || {
        Arbiter::spawn(
            stdin()
                .chain(futures::stream::once(Ok("eof".into())))
                .map_err(|e| eprintln!("err={}", e))
                .map(move |l| -> Box<dyn Future<Item = (), Error = ()>> {
                    use futures::future::{self, Either};
                    if l.starts_with("run ") {
                        Box::new(
                            pp.send(Exec {
                                executable: "bin/bash".into(),
                                args: vec!["-c".into(), (&l[4..]).into()],
                            })
                            .map_err(|e| eprintln!("err={}", e))
                            .and_then(|r| match r {
                                Ok((o, e)) => Ok(eprintln!("out=[{}], err=[{}]", o, e)),
                                Err(e) => Ok(eprintln!("error={}", e)),
                            }),
                        )
                    } else if l == "list" {
                        eprintln!("ask list");
                        Box::new(
                            pp.send(List)
                                .map_err(|e| eprintln!("err={}", e))
                                .and_then(|pids| Ok(eprintln!("pids={:?}", pids))),
                        )
                    } else if l.starts_with("stop ") {
                        let pid: Pid = (&l[5..]).parse().unwrap();
                        Box::new(
                            pp.send(Stop(pid))
                                .map_err(|e| eprintln!("err={}", e))
                                .and_then(|r| Ok(eprintln!("stop={:?}", r))),
                        )
                    } else {
                        Box::new(future::ok(eprintln!("line: [{}]", l)))
                    }
                })
                .buffer_unordered(100)
                .for_each(|_| Ok(()))
                .then(|_| Ok(System::current().stop())),
        )
    });
}
