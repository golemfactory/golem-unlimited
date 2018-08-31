extern crate gu_p2p;
extern crate actix;

#[macro_use]
extern crate serde_derive;

use actix::prelude::*;
use gu_p2p::rpc::*;

struct Test;

impl Actor for Test {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &'_ mut <Self as Actor>::Context) {
        println!("started!");
        ctx.bind::<Init>(1)
    }
}

#[derive(Serialize, Deserialize)]
struct Init;

impl Message for Init {
    type Result= ();
}

impl Handler<Init> for Test {
    type Result = ();

    fn handle(&mut self, msg: Init, ctx: &mut Self::Context) -> <Self as Handler<Init>>::Result {
        unimplemented!()
    }
}

fn main() {

    let sys= System::new("rpc-test");

    let a = start_actor(Test);

    let _ = sys.run();
    println!("ok");
}