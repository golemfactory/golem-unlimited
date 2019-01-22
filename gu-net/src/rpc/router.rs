use super::{error, message::*, util::*};
use actix::{fut, prelude::*};
use futures::prelude::*;
use std::{
    collections::HashMap,
    io::{self, Write},
};

pub struct MessageRouter {
    destinations: HashMap<DestinationId, Box<LocalEndpoint + 'static>>,
    reply_destinations: HashMap<DestinationId, Box<LocalReplyEndpoint + 'static>>,
    remotes: HashMap<NodeId, Recipient<EmitMessage<String>>>,
}

impl Drop for MessageRouter {
    fn drop(&mut self) {
        info!("router stopped");
    }
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

#[derive(Message)]
pub struct BindDestination {
    pub destination_id: DestinationId,
    pub endpoint: Box<LocalEndpoint + 'static + Send>,
}

#[derive(Message)]
pub struct BindReplyDestination {
    pub destination_id: DestinationId,
    pub endpoint: Box<LocalReplyEndpoint + 'static + Send>,
}

impl MessageRouter {}

impl Actor for MessageRouter {
    type Context = Context<Self>;
}

impl Default for MessageRouter {
    fn default() -> Self {
        MessageRouter {
            destinations: HashMap::new(),
            reply_destinations: HashMap::with_capacity(32),
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

pub trait LocalReplyEndpoint {
    fn handle(
        &mut self,
        message: RouteMessage<Result<String, TransportError>>,
        ctx: &mut <MessageRouter as Actor>::Context,
    );
}

impl Handler<RouteMessage<String>> for MessageRouter {
    type Result = ();

    fn handle(&mut self, msg: RouteMessage<String>, ctx: &mut Self::Context) -> Self::Result {
        //let destination = msg.destination.clone();
        debug!("handling dest: {:?}", msg.destination);
        if let Some(v) = self.destinations.get_mut(&msg.destination) {
            v.handle(msg, ctx);
        } else if let Some(r) = EmitMessage::reply(&msg, TransportResult::no_destination()) {
            error!("no dest: {:?}", msg.destination);
            ctx.notify(r);
        } else {
            error!("no dest: {:?} and no reply", msg.destination);
        }
    }
}

impl Handler<EmitMessage<String>> for MessageRouter {
    type Result = ActorResponse<MessageRouter, MessageId, error::Error>;

    fn handle(&mut self, msg: EmitMessage<String>, ctx: &mut Self::Context) -> Self::Result {
        let dest_node = msg.dest_node.clone();
        let f = if let Some(v) = self.remotes.get_mut(&msg.dest_node) {
            v.send(msg).then(|r| match r {
                Err(e) => {
                    error!("emit err: {}", e);
                    Err(e.into())
                }
                Ok(v) => v,
            })
        } else {
            error!("endpoint not connected: {:?}", msg.dest_node);
            return ActorResponse::reply(Err(error::ErrorKind::NotConnected.into()));
        };

        ActorResponse::async(
            f.into_actor(self)
                .map_err(move |e: error::Error, act, ctx| {
                    match e.kind() {
                        error::ErrorKind::MailBox(MailboxError::Closed) => {
                            error!("removing invalid destination node {:?}", &dest_node);
                            act.remotes.remove(&dest_node);
                        }
                        _ => (),
                    }

                    e
                }),
        )
    }
}

impl Handler<RouteMessage<Result<String, TransportError>>> for MessageRouter {
    type Result = ();

    fn handle(
        &mut self,
        msg: RouteMessage<Result<String, TransportError>>,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        debug!("handling dest: {:?}", msg.destination);
        if let Some(v) = self.reply_destinations.get_mut(&msg.destination) {
            v.handle(msg, ctx);
        } else {
            error!("no dest: {:?} and no reply", msg.destination);
        }
    }
}

impl Handler<BindDestination> for MessageRouter {
    type Result = ();

    fn handle(&mut self, msg: BindDestination, ctx: &mut Self::Context) -> Self::Result {
        debug!("registered: {:?}", &msg.destination_id);
        self.destinations.insert(msg.destination_id, msg.endpoint);
    }
}

impl Handler<BindReplyDestination> for MessageRouter {
    type Result = ();

    fn handle(
        &mut self,
        msg: BindReplyDestination,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<BindReplyDestination>>::Result {
        self.reply_destinations
            .insert(msg.destination_id, msg.endpoint);
    }
}

impl Handler<AddEndpoint> for MessageRouter {
    type Result = ();

    fn handle(
        &mut self,
        msg: AddEndpoint,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<AddEndpoint>>::Result {
        self.remotes.insert(msg.node_id, msg.recipient);
    }
}
