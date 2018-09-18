extern crate actix;
extern crate futures;
extern crate gu_hardware;
extern crate gu_p2p;
extern crate sysinfo;

use actix::Arbiter;
use futures::future::Future;
use gu_p2p::rpc::start_actor;
use std::path::PathBuf;

fn main() {
    actix::System::run(|| {
        let disk = start_actor(gu_hardware::disk::DiskActor::default());
        let gpu = start_actor(gu_hardware::gpu::GpuActor::default());
        let ram = start_actor(gu_hardware::ram::RamActor::default());

        Arbiter::spawn(
            ram.send(gu_hardware::ram::RamQuery)
                .then(|res| Ok(println!("{:?}", res))),
        );

        #[cfg(target_os = "linux")]
        Arbiter::spawn(
            gpu.send(gu_hardware::gpu::GpuQuery)
                .then(|res| Ok(println!("{:?}", res))),
        );

        Arbiter::spawn(
            disk.send(gu_hardware::disk::DiskQuery::new(PathBuf::from(
                "/boot/efi",
            ))).then(|res| Ok(println!("{:?}", res))),
        );
    });
}
