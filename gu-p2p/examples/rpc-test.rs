extern crate gu_actix;
extern crate gu_p2p;

extern crate actix;
extern crate actix_web;
extern crate env_logger;
extern crate futures;
extern crate rand;
extern crate serde_json;
extern crate smallvec;

#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

use actix::prelude::*;
use futures::unsync::oneshot;
use futures::{future, prelude::*};
use gu_actix::*;
use gu_p2p::rpc::*;
use gu_p2p::NodeId;
use std::collections::HashMap;

use actix_web::*;

struct Test {
    cnt: u32,
}

impl Actor for Test {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        info!("started!");
        ctx.bind::<Init>(1)
    }
}

#[derive(Serialize, Deserialize)]
struct Init;

impl Message for Init {
    type Result = u32;
}

impl Handler<Init> for Test {
    type Result = u32;

    fn handle(&mut self, msg: Init, ctx: &mut Self::Context) -> <Self as Handler<Init>>::Result {
        info!("init!");
        self.cnt += 1;
        self.cnt
    }
}

struct Callback {
    reply: HashMap<MessageId, oneshot::Sender<String>>,
    fake_node_id: NodeId,
    tx_map: HashMap<DestinationId, oneshot::Sender<Result<String, RpcError>>>,
}

impl Actor for Callback {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        info!("Callback actor started")
    }
}

impl ArbiterService for Callback {
    fn service_started(&mut self, ctx: &mut Context<Self>) {
        use gu_p2p::rpc::router::*;
        info!("Callback service started");
        MessageRouter::from_registry().do_send(AddEndpoint {
            node_id: self.fake_node_id.clone(),
            recipient: ctx.address().recipient(),
        })
    }
}

impl Handler<EmitMessage<String>> for Callback {
    type Result = Result<MessageId, RpcError>;

    fn handle(
        &mut self,
        msg: EmitMessage<String>,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<EmitMessage<String>>>::Result {
        info!("emit={:?}", &msg.body);
        if let Some(tx) = self.tx_map.remove(&msg.destination) {
            tx.send(msg.body.into()).unwrap();
        }

        Ok(gen_destination_id())
    }
}

impl Supervised for Callback {}

impl Default for Callback {
    fn default() -> Self {
        use rand::prelude::*;
        let mut rng = thread_rng();

        Callback {
            reply: HashMap::new(),
            fake_node_id: rng.gen(),
            tx_map: HashMap::new(),
        }
    }
}

struct Forward(RouteMessage<String>);

impl Message for Forward {
    type Result = Result<String, RpcError>;
}

impl Handler<Forward> for Callback {
    type Result = ActorResponse<Callback, String, RpcError>;

    fn handle(
        &mut self,
        mut msg: Forward,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<Forward>>::Result {
        msg.0.sender = self.fake_node_id;
        msg.0.reply_to = Some(gen_destination_id());
        let (tx, rx) = oneshot::channel();
        self.tx_map.insert(msg.0.reply_to.clone().unwrap(), tx);
        MessageRouter::from_registry().do_send(msg.0);
        ActorResponse::async(rx.flatten_fut().into_actor(self))
    }
}

fn do_test<S>(r: HttpRequest<S>) -> impl Responder {
    Callback::from_registry()
        .send(Forward(RouteMessage {
            msg_id: gen_destination_id(),
            sender: [0; 32],
            destination: public_destination(1),
            reply_to: None,
            correlation_id: None,
            ts: 0,
            expires: None,
            body: String::from("null"),
        }))
        .flatten_fut()
        .and_then(|r| future::ok(r))
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("{}", e)))
        .responder()
}

fn main() {
    if ::std::env::var("RUST_LOG").is_err() {
        ::std::env::set_var("RUST_LOG", "info,gu_p2p=debug")
    }
    env_logger::init();

    let sys = System::new("rpc-test");

    let a = start_actor(Test { cnt: 0 });

    info!("init={}", serde_json::to_string_pretty(&Init).unwrap());

    info!("test={:?}", gen_destination_id());
    info!("test={:?}", gen_destination_id());
    info!("test={:?}", gen_destination_id());

    server::new(move || App::new().route("/test", http::Method::GET, do_test))
        .bind("127.0.0.1:6767")
        .unwrap()
        .start();

    let _ = sys.run();
    println!("ok");
}
