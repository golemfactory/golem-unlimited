use actix::{Actor, ArbiterService, Handler, Message};
use futures::Future;
use gu_p2p::rpc::RemotingContext;
use sysinfo::SystemExt;

use actix::ActorFuture;
use actix::ActorResponse;
use actix::Addr;
use actix::WrapFuture;
use actix_web::Error;
use disk::{DiskInfo, DiskQuery};
use futures::future;
use gpu::{GpuCount, GpuQuery};
use gu_actix::flatten::FlattenFuture;
use inner_actor::InnerActor;
use ram::{RamInfo, RamQuery};

#[derive(Debug, Serialize, Deserialize)]
pub struct HardwareQuery;

impl HardwareQuery {
    const ID: u32 = 19354;
}

impl Default for HardwareQuery {
    fn default() -> Self {
        Self {
        }
    }
}

enum Info {
    Gpu(GpuCount),
    Ram(RamInfo),
    Disk(DiskInfo),
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
    type Result = Result<Hardware, ()>;
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

fn gpu(query: GpuQuery, inner: &Addr<InnerActor>) -> impl Future<Item = GpuCount, Error = ()> {
        inner
            .send(query)
            .flatten_fut()
            .map_err(|_| ()
    )
}

fn ram(query: RamQuery, inner: &Addr<InnerActor>) -> impl Future<Item = RamInfo, Error = ()> {
        inner
            .send(query)
            .flatten_fut()
            .map_err(|_| ())
}

fn disk(query: DiskQuery, inner: &Addr<InnerActor>) -> Box<Future<Item = Info, Error = ()>> {
    Box::new(
        inner
            .send(query)
            .flatten_fut()
            .and_then(|res| Ok(Info::Disk(res)))
            .map_err(|_| ()),
    )
}

impl Handler<HardwareQuery> for HardwareActor {
    type Result = ActorResponse<Self, Hardware, ()>;

    fn handle(
        &mut self,
        msg: HardwareQuery,
        _ctx: &mut RemotingContext<Self>,
    ) -> <Self as Handler<HardwareQuery>>::Result {
        let inner = InnerActor::from_registry();

        ActorResponse::async(
            gpu(GpuQuery::default(), &inner).join(ram(RamQuery::default(), &inner))
                .and_then(|(gpu, ram)| Ok(Hardware {
                    gpu: Some(gpu), ram: Some(ram),
                    disk: None,
                })
                    .map_err(|_ : ((),())| ()))
                .into_actor(self)

        )
    }
}


