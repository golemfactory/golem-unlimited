use super::context::RemotingContext;
use super::message::{DestinationId, MessageId, RouteMessage};
use super::router::MessageRouter;
use actix::prelude::*;
use futures::unsync::oneshot;
use std::collections::HashMap;

pub struct ReplyRouter {
    router: Addr<MessageRouter>,
    destination_id: Option<DestinationId>,
    reply_map: HashMap<MessageId, oneshot::Sender<RouteMessage<String>>>,
}

impl Actor for ReplyRouter {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.destination_id = None;
    }
}

impl Supervised for ReplyRouter {}

impl ArbiterService for ReplyRouter {}

impl Default for ReplyRouter {
    fn default() -> Self {
        info!("new router");
        ReplyRouter {
            router: MessageRouter::from_registry(),
            destination_id: None,
            reply_map: HashMap::new(),
        }
    }
}

impl<T> Handler<RouteMessage<T>> for ReplyRouter {
    type Result = ();

    fn handle(&mut self, msg: RouteMessage<T>, ctx: &mut Self::Context) -> Self::Result {}
}
