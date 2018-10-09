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
use gu_p2p::rpc::*;

use actix_web::*;

struct Test {
    cnt: u32,
}

impl Actor for Test {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        info!("started!");
        ctx.bind::<Init>(1);
        ctx.bind::<Echo>(2);
    }
}

#[derive(Serialize, Deserialize)]
struct Init;

impl Message for Init {
    type Result = u32;
}

impl Handler<Init> for Test {
    type Result = u32;

    fn handle(&mut self, _msg: Init, _ctx: &mut Self::Context) -> <Self as Handler<Init>>::Result {
        info!("init!");
        self.cnt += 1;
        self.cnt
    }
}

#[derive(Serialize, Deserialize)]
struct Echo(serde_json::Value);

impl Message for Echo {
    type Result = Result<serde_json::Value, ()>;
}

impl Handler<Echo> for Test {
    type Result = Result<serde_json::Value, ()>;

    fn handle(&mut self, msg: Echo, _ctx: &mut Self::Context) -> <Self as Handler<Echo>>::Result {
        Ok(msg.0)
    }
}

fn main() {
    if ::std::env::var("RUST_LOG").is_err() {
        ::std::env::set_var("RUST_LOG", "info,gu_p2p=debug")
    }
    env_logger::init();

    let sys = System::new("rpc-test");

    let _a = start_actor(Test { cnt: 0 });

    info!("init={}", serde_json::to_string_pretty(&Init).unwrap());

    info!("test={:?}", gen_destination_id());
    info!("test={:?}", gen_destination_id());
    info!("test={:?}", gen_destination_id());

    server::new(move || App::new().scope("/m", mock::scope))
        .bind("127.0.0.1:6767")
        .unwrap()
        .start();

    let _ = sys.run();
    println!("ok");
}
