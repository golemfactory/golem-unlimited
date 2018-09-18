use actix::{Actor, Context, ArbiterService, Supervised, Handler, ActorResponse};
use ram::{RamInfo, RamQuery, ram_info};
use gpu::{GpuCount, GpuQuery, discover_gpu_vendors};
use disk::{DiskInfo, DiskQuery, disk_info};
use error::Error;
use sysinfo::{self, SystemExt};

#[derive(Default)]
pub struct HardwareActor {
    sys: sysinfo::System,
}

impl Actor for HardwareActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.sys = sysinfo::System::new();
    }
}

impl Supervised for HardwareActor {}
impl ArbiterService for HardwareActor {}

impl Handler<RamQuery> for HardwareActor {
    type Result = ActorResponse<HardwareActor, RamInfo, Error>;

    fn handle(&mut self, _msg: RamQuery, _ctx: &mut Context<Self>) -> <Self as Handler<RamQuery>>::Result {
        ActorResponse::reply(Ok(ram_info(&self.sys)))
    }
}

impl Handler<GpuQuery> for HardwareActor {
    type Result = ActorResponse<HardwareActor, GpuCount, Error>;

    fn handle(&mut self, _msg: GpuQuery, _ctx: &mut Context<Self>) -> <Self as Handler<GpuQuery>>::Result {
        ActorResponse::reply(discover_gpu_vendors())
    }
}

impl Handler<DiskQuery> for HardwareActor {
    type Result = ActorResponse<HardwareActor, DiskInfo, Error>;

    fn handle(&mut self, msg: DiskQuery, _ctx: &mut Context<Self>) -> <Self as Handler<DiskQuery>>::Result {
        ActorResponse::reply(disk_info(&self.sys, msg.path()))
    }
}


