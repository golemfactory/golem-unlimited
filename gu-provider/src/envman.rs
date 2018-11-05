//! Execution environment manager.
//!


use actix::prelude::*;
use gu_net::rpc::{RemotingContext, RemotingSystemService};
use gu_envman_api::*;

/// Actor
struct EnvMan {
    _inner: (),
}

impl Default for EnvMan {
    fn default() -> Self {
        Self { _inner: () }
    }
}

impl Actor for EnvManService {
    type Context = RemotingContext<Self>;
}

pub trait EnvManService : Handler<CreateSession> + Handler<SessionUpdate> + Handler<GetSessions> + Handler<DestroySession> {

}


struct Register<T> where T : Actor + EnvManService {
    address : Addr<T>
}

