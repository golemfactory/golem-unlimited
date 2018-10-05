extern crate actix;
extern crate env_logger;
extern crate futures;
extern crate gu_lan;
extern crate log;

use actix::prelude::*;
use env_logger::Builder;
use futures::future;
use futures::Future;
use gu_lan::actor::Continuous;
use gu_lan::actor::SubscribeInstance;
use gu_lan::continuous::NewInstance;
use log::LevelFilter;

struct Receiver;

impl Actor for Receiver {
    type Context = Context<Self>;
}

impl Handler<NewInstance> for Receiver {
    type Result = ();

    fn handle(&mut self, msg: NewInstance, _ctx: &mut Context<Self>) -> () {
        println!("{:?}", msg.data);
    }
}

fn main() {
    Builder::from_default_env()
        .filter_level(LevelFilter::Off)
        .init();

    let sys = actix::System::new("none_example");
    let actor = gu_lan::actor::MdnsActor::<Continuous>::new();
    let address = actor.start();
    let receiver = Receiver.start();

    let res = address.send(SubscribeInstance {
        service: gu_lan::service::ServiceDescription::new("gu-hub", "_unlimited._tcp"),
        rec: receiver.recipient(),
    });

    Arbiter::spawn(res.then(|res| {
        match res {
            Ok(result) => println!("Received result: {:?}", result),
            _ => println!("Something went wrong"),
        }

        future::result(Ok(()))
    }));

    let _ = sys.run();
}
