use actix::Actor;
use actix::ActorResponse;
use actix::Handler;
use actix::Message;
use error::Error;
use error::Result;
use gu_p2p::rpc::RemotingContext;
use sysinfo::SystemExt;

#[derive(Debug, Serialize, Deserialize)]
pub struct RamInfo {
    free: u64,
    used: u64,
    total: u64,
}

impl RamInfo {
    pub fn free(&self) -> u64 {
        self.free
    }

    pub fn used(&self) -> u64 {
        self.used
    }

    pub fn total(&self) -> u64 {
        self.total
    }
}

pub(crate) fn ram_info(sys: &impl SystemExt) -> RamInfo {
    RamInfo {
        free: sys.get_free_memory(),
        used: sys.get_used_memory(),
        total: sys.get_total_memory(),
    }
}

#[derive(Debug, Default)]
pub struct RamQuery;

impl Message for RamQuery {
    type Result = Result<RamInfo>;
}

#[derive(Default)]
pub struct RamActor;

impl Actor for RamActor {
    type Context = RemotingContext<Self>;
}

impl Handler<RamQuery> for RamActor {
    type Result = ActorResponse<Self, RamInfo, Error>;

    fn handle(
        &mut self,
        msg: RamQuery,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<RamQuery>>::Result {
        use actix::{ArbiterService, WrapFuture};
        use actor::HardwareActor;
        use gu_actix::FlattenFuture;

        ActorResponse::async(
            HardwareActor::from_registry()
                .send(msg)
                .flatten_fut()
                .into_actor(self),
        )
    }
}
