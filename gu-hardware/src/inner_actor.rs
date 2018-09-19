use actix::MessageResult;
use actix::{Actor, ArbiterService, Context, Handler, Supervised};
use sysinfo::{self, SystemExt};

use disk::{disk_info, DiskQuery};
use gpu::{discover_gpu_vendors, GpuQuery};
use ram::{ram_info, RamQuery};

#[derive(Default)]
pub struct InnerActor {
    sys: sysinfo::System,
}

impl Actor for InnerActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.sys = sysinfo::System::new();
    }
}

impl Supervised for InnerActor {}
impl ArbiterService for InnerActor {}

impl Handler<RamQuery> for InnerActor {
    type Result = MessageResult<RamQuery>;

    fn handle(
        &mut self,
        _msg: RamQuery,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<RamQuery>>::Result {
        MessageResult(Ok(ram_info(&self.sys)))
    }
}

impl Handler<DiskQuery> for InnerActor {
    type Result = MessageResult<DiskQuery>;

    fn handle(
        &mut self,
        msg: DiskQuery,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<DiskQuery>>::Result {
        MessageResult(disk_info(&self.sys, msg.path()))
    }
}

impl Handler<GpuQuery> for InnerActor {
    type Result = MessageResult<GpuQuery>;

    fn handle(
        &mut self,
        msg: GpuQuery,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<GpuQuery>>::Result {
        #[cfg(target_os = "linux")]
        return MessageResult(discover_gpu_vendors().map(|a| Some(a)));
        #[cfg(not(target_os = "linux"))]
        return None;
    }
}
