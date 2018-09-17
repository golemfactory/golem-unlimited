use actix::{Actor, Context, ArbiterService, Supervised, Handler, ActorResponse};
use ram::{RamInfo, RamQuery, ram_info};
use gpu::{GpuCount, GpuQuery, discover_gpu_vendors};
//use disk;

use error::Error;

#[derive(Default)]
pub struct HardwareActor {}

impl HardwareActor {
    pub fn new() -> Self {
         Self::default()
    }
}

impl Actor for HardwareActor {
    type Context = Context<Self>;
}

impl Supervised for HardwareActor {}
impl ArbiterService for HardwareActor {}

impl Handler<RamQuery> for HardwareActor {
    type Result = ActorResponse<HardwareActor, RamInfo, Error>;

    fn handle(&mut self, _msg: RamQuery, _ctx: &mut Context<Self>) -> <Self as Handler<RamQuery>>::Result {
        ActorResponse::reply(Ok(ram_info()))
    }
}

impl Handler<GpuQuery> for HardwareActor {
    type Result = ActorResponse<HardwareActor, GpuCount, Error>;

    fn handle(&mut self, _msg: GpuQuery, _ctx: &mut Context<Self>) -> <Self as Handler<GpuQuery>>::Result {
        ActorResponse::reply(discover_gpu_vendors())
    }
}



