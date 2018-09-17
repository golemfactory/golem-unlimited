extern crate gu_hardware;
extern crate sysinfo;
extern crate actix;
extern crate futures;

use futures::future::Future;
use actix::{Actor, Arbiter};

fn main() {
    let sys = actix::System::new("Hardware discovery");
    let actor = gu_hardware::actor::HardwareActor::new();
    let address = actor.start();

    Arbiter::spawn(address
        .send(gu_hardware::ram::RamQuery::new())
        .then(|res| Ok(println!("{:?}", res)))
    );

    Arbiter::spawn(address
        .send(gu_hardware::gpu::GpuQuery::new())
        .then(|res| Ok(println!("{:?}", res)))
    );

    let _ = sys.run();
}