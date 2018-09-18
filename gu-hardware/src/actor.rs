use actix::MessageResult;
use actix::{Actor, ArbiterService, Context, Handler, Supervised};
use sysinfo::{self, SystemExt};

use disk::{disk_info, DiskQuery};
#[cfg(target_os = "linux")]
use gpu::{discover_gpu_vendors, GpuQuery};
use ram::{ram_info, RamQuery};

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
    type Result = MessageResult<RamQuery>;

    fn handle(
        &mut self,
        _msg: RamQuery,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<RamQuery>>::Result {
        MessageResult(Ok(ram_info(&self.sys)))
    }
}

impl Handler<DiskQuery> for HardwareActor {
    type Result = MessageResult<DiskQuery>;

    fn handle(
        &mut self,
        msg: DiskQuery,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<DiskQuery>>::Result {
        MessageResult(disk_info(&self.sys, msg.path()))
    }
}
