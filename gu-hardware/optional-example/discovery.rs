extern crate gu_hardware;
extern crate sysinfo;
extern crate actix;
extern crate futures;
extern crate gu_actix;
extern crate gu_p2p;

use futures::prelude::*;
use actix::prelude::*;
use gu_actix::prelude::*;
use gu_hardware as guh;
use std::path::PathBuf;
use gu_p2p::rpc::start_actor;

fn main() {

    System::run(|| {
        let disk = start_actor(gu_hardware::disk::DiskActor::default());
        let gpu = start_actor(gu_hardware::gpu::GpuActor::default());
        let ram = start_actor(gu_hardware::ram::RamActor::default());


        Arbiter::spawn(
        ram.send(guh::ram::RamQuery)
            .flatten_fut()
            .then(|res| Ok::<(), ()>(println!("{:?}", res)))
            .join3(
                gpu.send(guh::gpu::GpuQuery)
                    .flatten_fut()
                    .then(|res| Ok(println!("{:?}", res))),
                disk.send(guh::disk::DiskQuery::new(PathBuf::from("/boot/efi")))
                    .flatten_fut()
                    .then(|res| Ok(println!("{:?}", res)))
            ).then(|_| Ok(System::current().stop())))

    });

}