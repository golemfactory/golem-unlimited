extern crate gu_hardware;
extern crate sysinfo;
extern crate actix;
extern crate futures;
extern crate gu_actix;

use futures::prelude::*;
use actix::prelude::*;
use gu_actix::prelude::*;
use gu_hardware::actor::HardwareActor;
use gu_hardware as guh;
use std::path::PathBuf;

fn main() {

    System::run(|| {
        let hardware = HardwareActor::from_registry();

        Arbiter::spawn(
        hardware.send(guh::ram::RamQuery::new())
            .flatten_fut()
            .then(|res| Ok::<(), ()>(println!("{:?}", res)))
            .join3(
                hardware.send(guh::gpu::GpuQuery::new())
                    .flatten_fut()
                    .then(|res| Ok(println!("{:?}", res))),
                hardware.send(guh::disk::DiskQuery::new(PathBuf::from("/boot/efi")))
                    .flatten_fut()
                    .then(|res| Ok(println!("{:?}", res)))
            ).then(|_| Ok(System::current().stop())))

    });

}