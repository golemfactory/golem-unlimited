use super::error;
use super::message::*;
use super::util::*;
use actix::prelude::*;
use futures::prelude::*;
use std::collections::HashMap;
use std::io::{self, Write};

pub struct MessageRouter {
    destinations: HashMap<DestinationId, Box<LocalEndpoint + 'static>>,
    remotes: HashMap<NodeId, Recipient<EmitMessage<String>>>,
}

#[derive(Message)]
pub struct AddEndpoint {
    pub node_id: NodeId,
    pub recipient: Recipient<EmitMessage<String>>,
}

#[derive(Message)]
pub struct DelEndpoint {
    pub node_id: NodeId,
}

pub struct BindDestination {
    pub destination_id: DestinationId,
    pub endpoint: Box<LocalEndpoint + 'static + Send>,
}

impl Message for BindDestination {
    type Result = ();
}

impl MessageRouter {}

impl Actor for MessageRouter {
    type Context = Context<Self>;
}

impl Default for MessageRouter {
    fn default() -> Self {
        MessageRouter {
            destinations: HashMap::new(),
            remotes: HashMap::new(),
        }
    }
}

impl Supervised for MessageRouter {}

impl SystemService for MessageRouter {}

pub trait LocalEndpoint {
    fn handle(
        &mut self,
        message: RouteMessage<String>,
        ctx: &mut <MessageRouter as Actor>::Context,
    );
}

impl Handler<RouteMessage<String>> for MessageRouter {
    type Result = ();

    fn handle(&mut self, msg: RouteMessage<String>, ctx: &mut Self::Context) -> Self::Result {
        //let destination = msg.destination.clone();
        if let Some(v) = self.destinations.get_mut(&msg.destination) {
            v.handle(msg, ctx);
        } else if let Some(r) = EmitMessage::reply(&msg, TransportResult::NoDestination) {
            ctx.notify(r)
        }
    }
}

impl Handler<EmitMessage<String>> for MessageRouter {
    type Result = ActorResponse<MessageRouter, MessageId, error::Error>;

    fn handle(&mut self, msg: EmitMessage<String>, ctx: &mut Self::Context) -> Self::Result {
        let f = if let Some(v) = self.remotes.get_mut(&msg.dest_node) {
            v.send(msg).then(|r| match r {
                Err(e) => Err(e.into()),
                Ok(v) => v,
            })
        } else {
            return ActorResponse::reply(Err(error::ErrorKind::NotConnected.into()));
        };

        ActorResponse::async(f.into_actor(self))
    }
}

impl Handler<BindDestination> for MessageRouter {
    type Result = ();

    fn handle(&mut self, msg: BindDestination, ctx: &mut Self::Context) -> Self::Result {
        println!("registed: {:?}", &msg.destination_id);
        self.destinations.insert(msg.destination_id, msg.endpoint);

    }
}