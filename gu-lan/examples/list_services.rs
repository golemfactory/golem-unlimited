extern crate actix;
extern crate env_logger;
extern crate futures;
extern crate gu_lan;
extern crate log;

use actix::prelude::*;
use env_logger::Builder;
use futures::future;
use futures::Future;
use log::LevelFilter;

fn main() {
    Builder::from_default_env()
        .filter_level(LevelFilter::Off)
        .init();

    let sys = actix::System::new("none_example");
    let actor = gu_lan::resolve_actor::ResolveActor::new();
    let address = actor.start();
    let res = address.send(gu_lan::service::Service::new("gu-hub", "_unlimited._tcp"));

    Arbiter::spawn(res.then(|res| {
        match res {
            Ok(result) => println!("Received result: {:?}", result),
            _ => println!("Something went wrong"),
        }

        future::result(Ok(()))
    }));

    let _ = sys.run();
}
