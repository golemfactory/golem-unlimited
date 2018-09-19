use actix::{
    Actor, ActorFuture, ActorResponse, Addr, ArbiterService, Handler, Message, WrapFuture,
};
use actix_web::Error;
use futures::future;
use futures::Future;
use sysinfo::SystemExt;

use disk::{DiskInfo, DiskQuery};
use gpu::{GpuCount, GpuQuery};
use gu_actix::flatten::FlattenFuture;
use gu_p2p::rpc::RemotingContext;
use inner_actor::InnerActor;
use ram::{RamInfo, RamQuery};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HardwareQuery;

impl HardwareQuery {
    const ID: u32 = 19354;
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Hardware {
    #[serde(skip_serializing_if = "Option::is_none")]
    gpu: Option<GpuCount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ram: Option<RamInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disk: Option<DiskInfo>,
}

impl Message for HardwareQuery {
    type Result = Result<Hardware, String>;
}

#[derive(Debug, Default)]
pub struct HardwareActor {}

impl HardwareActor {
    fn new() -> Self {
        Self::default()
    }
}

impl Actor for HardwareActor {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.bind::<HardwareQuery>(HardwareQuery::ID);
    }
}

fn gpu(
    query: GpuQuery,
    inner: &Addr<InnerActor>,
) -> impl Future<Item = Option<GpuCount>, Error = String> {
    inner.send(query).flatten_fut().map_err(|e| format!("{}", e))
}

fn ram(
    query: RamQuery,
    inner: &Addr<InnerActor>,
) -> impl Future<Item = Option<RamInfo>, Error = String> {
    inner
        .send(query)
        .flatten_fut()
        .map_err(|e| format!("{}", e))
        .and_then(|r| Ok(Some(r)))
}

fn disk(
    query: DiskQuery,
    inner: &Addr<InnerActor>,
) -> impl Future<Item = Option<DiskInfo>, Error = String> {
    inner
        .send(query)
        .flatten_fut()
        .map_err(|e| format!("{}", e))
        .then(|r| Ok(r.ok()))
}

impl Handler<HardwareQuery> for HardwareActor {
    type Result = ActorResponse<Self, Hardware, String>;

    fn handle(
        &mut self,
        msg: HardwareQuery,
        _ctx: &mut RemotingContext<Self>,
    ) -> <Self as Handler<HardwareQuery>>::Result {
        let inner = InnerActor::from_registry();

        ActorResponse::async(
            gpu(GpuQuery::default(), &inner)
                .join3(
                    ram(RamQuery::default(), &inner),
                    disk(DiskQuery::new(), &inner),
                ).and_then(|(gpu, ram, disk)| {
                    Ok(Hardware { gpu, ram, disk })
                }).into_actor(self),
        )
    }
}
