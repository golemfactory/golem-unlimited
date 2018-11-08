extern crate actix;
extern crate futures;
extern crate gu_persist;

use actix::prelude::*;
use futures::prelude::*;
use std::{borrow::Cow, env};

enum Cmd {
    Fetch(String),
    Put(String, String),
}

struct MyActor(Cmd);

impl Actor for MyActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("I am alive!");
        let fs = SyncArbiter::start(1, || {
            gu_persist::file_storage::FileStorage::from_path("/tmp/test")
        });

        let l: Box<Future<Item = (), Error = ()>> = match &self.0 {
            Cmd::Fetch(key) => Box::new(
                fs.send(gu_persist::storage::Fetch(Cow::Owned(key.clone())))
                    .and_then(|v| {
                        match v {
                            Ok(b) => println!("bytes: {:?}", b),
                            Err(e) => println!("err {:?}", e),
                        }
                        println!("stop app");
                        System::current().stop();
                        Ok(())
                    }).map_err(|_| ()),
            ),
            Cmd::Put(key, v) => Box::new(
                fs.send(gu_persist::storage::Put(
                    Cow::Owned(key.clone()),
                    v.bytes().collect(),
                )).and_then(|v| {
                    match v {
                        Ok(_b) => println!("ok"),
                        Err(e) => println!("err {:?}", e),
                    }
                    println!("stop app");
                    System::current().stop();
                    Ok(())
                }).map_err(|_| ()),
            ),
        };

        ctx.spawn(l.into_actor(self));
        // <- stop system
    }
}

fn main() {
    let sys = actix::System::new("test-config");

    let args: Vec<String> = env::args().collect();

    let _ = match args.len() {
        1 => MyActor(Cmd::Fetch("server".into())),
        2 => MyActor(Cmd::Fetch(args[1].clone())),
        3 => MyActor(Cmd::Put(args[1].clone(), args[2].clone())),
        _ => panic!("arr!"),
    }.start();

    let _ = sys.run();
}
