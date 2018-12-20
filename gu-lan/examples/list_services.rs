extern crate actix;
extern crate env_logger;
extern crate futures;
extern crate gu_actix;
extern crate gu_lan;
extern crate log;

use actix::{fut, prelude::*};
use env_logger::Builder;
use futures::Future;
use gu_actix::flatten::FlattenFuture;
use gu_lan::{
    actor::{Continuous, SubscribeInstance},
    NewInstance, ServiceDescription, Subscription,
};
use log::LevelFilter;

#[derive(Default)]
struct Receiver {
    set: Vec<Subscription>,
}

impl Actor for Receiver {
    type Context = Context<Self>;
}

impl Handler<NewInstance> for Receiver {
    type Result = ();

    fn handle(&mut self, msg: NewInstance, _ctx: &mut Context<Self>) -> () {
        println!("{:?}", msg.data);
    }
}

struct SubMessage {
    pub service: ServiceDescription,
}

impl Message for SubMessage {
    type Result = ();
}

impl Handler<SubMessage> for Receiver {
    type Result = ();

    fn handle(&mut self, msg: SubMessage, ctx: &mut Context<Self>) -> () {
        let cont = gu_lan::actor::MdnsActor::<Continuous>::from_registry();
        let rec = ctx.address().recipient();

        ctx.spawn(
            cont.send(SubscribeInstance {
                service: msg.service,
                rec,
            })
            .flatten_fut()
            .map_err(|_| ())
            .into_actor(self)
            .and_then(|res, act, _ctx| fut::ok(act.set.push(res))),
        );
    }
}

struct UnsubMessage {}

impl Message for UnsubMessage {
    type Result = ();
}

impl Handler<UnsubMessage> for Receiver {
    type Result = ();

    fn handle(&mut self, _msg: UnsubMessage, _ctx: &mut Context<Self>) -> () {
        self.set.pop();
    }
}

fn main() {
    Builder::from_default_env()
        .filter_level(LevelFilter::Error)
        .init();

    System::run(move || {
        let receiver = Receiver::default().start();

        Arbiter::spawn(
            receiver
                .send(SubMessage {
                    service: gu_lan::ServiceDescription::new("gu-provider", "_unlimited._tcp"),
                })
                .then(|_| Ok(())),
        );

        Arbiter::spawn(
            receiver
                .send(SubMessage {
                    service: gu_lan::ServiceDescription::new("gu-hub", "_unlimited._tcp"),
                })
                .then(|_| Ok(())),
        );
    });
}
