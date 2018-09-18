extern crate gu_hardware;
extern crate sysinfo;
extern crate actix;
extern crate futures;

use futures::future::Future;
use actix::{Arbiter, ArbiterService};
use std::path::PathBuf;

fn main() {
    let sys = actix::System::new("Hardware discovery");
    let address = gu_hardware::actor::HardwareActor::from_registry();

    Arbiter::spawn(address
        .send(gu_hardware::ram::RamQuery::new())
        .then(|res| Ok(println!("{:?}", res)))
    );

    Arbiter::spawn(address
        .send(gu_hardware::gpu::GpuQuery::new())
        .then(|res| Ok(println!("{:?}", res)))
    );

    Arbiter::spawn(address
        .send(gu_hardware::disk::DiskQuery::new(PathBuf::from("/boot/efi")))
        .then(|res| Ok(println!("{:?}", res)))
    );

    let _ = sys.run();
}