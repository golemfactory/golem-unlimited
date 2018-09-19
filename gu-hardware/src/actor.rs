use actix::{Actor, ArbiterService, Handler, Message};
use futures::Future;
use gu_p2p::rpc::RemotingContext;
use sysinfo::{self, SystemExt};

use actix::ActorResponse;
use actix::WrapFuture;
use disk::{DiskInfo, DiskQuery};
use error::{Error, Result};
use futures::future;
use gpu::{GpuCount, GpuQuery};
use gu_actix::flatten::FlattenFuture;
use inner_actor::InnerActor;
use ram::{RamInfo, RamQuery};
use actix::Addr;

#[derive(Debug)]
pub struct HardwareQuery {
    #[cfg(target_os = "linux")]
    gpu: Option<GpuQuery>,
    ram: Option<RamQuery>,
    disk: Option<DiskQuery>,
}

impl Default for HardwareQuery {
    fn default() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            gpu: Some(GpuQuery),
            ram: Some(RamQuery),
            disk: Some(DiskQuery::new("/".into())),
        }
    }
}

enum Info {
    Gpu(GpuCount),
    Ram(RamInfo),
    Disk(DiskInfo),
}

#[derive(Debug, Default)]
pub struct Hardware {
    #[cfg(target_os = "linux")]
    gpu: Option<GpuCount>,
    ram: Option<RamInfo>,
    disk: Option<DiskInfo>,
}

impl Message for HardwareQuery {
    type Result = Result<Hardware>;
}

#[derive(Debug, Default)]
pub struct HardwareActor {}

impl Actor for HardwareActor {
    type Context = RemotingContext<Self>;
}

fn gpu(query: GpuQuery, inner: &Addr<InnerActor>) -> Box<Future<Item=Info, Error=Error>> {
    Box::new(inner
            .send(query)
            .flatten_fut()
            .and_then(|res| Ok(Info::Gpu(res)))
    )
}

fn ram(query: RamQuery, inner: &Addr<InnerActor>) -> Box<Future<Item=Info, Error=Error>> {
    Box::new(inner
        .send(query)
        .flatten_fut()
        .and_then(|res| Ok(Info::Ram(res)))
    )
}

fn disk(query: DiskQuery, inner: &Addr<InnerActor>) -> Box<Future<Item=Info, Error=Error>> {
    Box::new(inner
        .send(query)
        .flatten_fut()
        .and_then(|res| Ok(Info::Disk(res)))
    )
}

impl Handler<HardwareQuery> for HardwareActor {
    type Result = ActorResponse<Self, Hardware, Error>;

    fn handle(
        &mut self,
        msg: HardwareQuery,
        _ctx: &mut RemotingContext<Self>,
    ) -> <Self as Handler<HardwareQuery>>::Result {
        let inner = InnerActor::from_registry();
        let mut vec = Vec::new();

        if let Some(v) = msg.gpu {
            vec.push(gpu(v, &inner))
        }
        if let Some(v) = msg.ram {
            vec.push(ram(v, &inner))
        }
        if let Some(v) = msg.disk {
            vec.push(disk(v, &inner))
        }

        ActorResponse::async(
            future::join_all(vec)
                .and_then(|vec| {
                    let mut result = Hardware::default();
                    for elt in vec {
                        match elt {
                            Info::Gpu(r) => result.gpu = Some(r),
                            Info::Ram(r) => result.ram = Some(r),
                            Info::Disk(r) => result.disk = Some(r),
                        }
                    }
                    Ok(result)
                })
                .into_actor(self),
        )
    }
}
