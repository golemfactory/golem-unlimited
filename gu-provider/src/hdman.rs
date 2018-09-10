use actix::prelude::*;
use gu_p2p::rpc::*;
use std::collections::HashMap;
use std::process::Command;

pub struct HdMan {
//    sessions: HashMap<String, HdManSession>,
}

impl Actor for HdMan {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.bind::<CreateSession>(CreateSession::ID);
    }
}

#[derive(Serialize, Deserialize)]
struct CreateSession {
    executable: String,
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
        println!("hey! I'm executing: {}", msg.executable);
        let output = Command::new(msg.executable).output().unwrap();
        if output.status.success() {
            println!("stdout: |{}|\nstderr: |{}|",
                     String::from_utf8_lossy(&output.stdout),
                     String::from_utf8_lossy(&output.stderr));
        }
        Err(())
    }
}

pub fn start() -> Addr<HdMan> {
    start_actor(HdMan {})
}
