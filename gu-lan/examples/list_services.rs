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


    System::run(move || {
        let cont = gu_lan::actor::MdnsActor::<Continuous>::from_registry();
        let receiver = Receiver.start();
        Arbiter::spawn(
            cont.send(SubscribeInstance {
                service: gu_lan::service::ServiceDescription::new("gu-provider", "_unlimited._tcp"),
                rec: receiver.recipient(),
        }).then(|_| Ok(())))
    });
}
