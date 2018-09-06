use actix::prelude::*;
use gu_p2p::rpc::*;

pub struct HdMan;

impl Actor for HdMan {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.bind::<CreateSession>(CreateSession::ID);
    }
}

#[derive(Serialize, Deserialize)]
struct CreateSession {
    url: String,
}

impl CreateSession {
    const ID: u32 = 200;
}

impl Message for CreateSession {
    type Result = Result<String, ()>;
}

impl Handler<CreateSession> for HdMan {
    type Result = Result<String, ()>;

    fn handle(
        &mut self,
        msg: CreateSession,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<CreateSession>>::Result {
        println!("hey! got url={}", msg.url);
        Err(())
    }
}

pub fn start() -> Addr<HdMan> {
    start_actor(HdMan)
}
