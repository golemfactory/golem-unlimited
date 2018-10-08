use actix::MessageResult;
use actix::{Actor, ArbiterService, Context, Handler, Supervised};
use sysinfo::{self, System, SystemExt};

use disk::{disk_info, DiskQuery};
use ram::{ram_info, RamQuery};

pub struct InnerActor {
    sys: System,
}

impl Default for InnerActor {
    fn default() -> InnerActor {
        InnerActor{ sys: sysinfo::System::new() }
    }
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
        self.sys.refresh_system();
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
        self.sys.refresh_disks();
        MessageResult(disk_info(&self.sys, msg.path()))
    }
}
