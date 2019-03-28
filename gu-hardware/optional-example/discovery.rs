use actix::prelude::*;
use gu_net::rpc::start_actor;

//use std::path::PathBuf;
use futures::prelude::*;
use gu_hardware::actor::{HardwareActor, HardwareQuery, StorageQuery};

fn main() {
    System::run(|| {
        let actor = start_actor(HardwareActor::default());
        let query = StorageQuery::new("/".into());

        Arbiter::spawn(
            actor
                .send(query)
                .map_err(|e| format!("{}", e))
                .and_then(|a| a)
                .and_then(|res| Ok(println!("{:?}", res)))
                .then(|_| Ok(System::current().stop())),
        );
    });
}
