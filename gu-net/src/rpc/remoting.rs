use super::super::types::NodeId;
use super::message::{public_destination, DestinationId};
use super::reply::{self, CallRemote, ReplyRouter};
use actix::prelude::*;
use futures::prelude::*;
use gu_actix::prelude::*;
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;

pub trait PublicMessage: Message {
    const ID: u32;
}

pub fn peer(node_id: NodeId) -> RemoteNode {
    RemoteNode(node_id)
}

pub struct RemoteNode(NodeId);

impl RemoteNode {
    pub fn with_destination<T>(&self, destination_id: DestinationId) -> RemoteEndpoint<T> {
        RemoteEndpoint {
            node_id: self.0,
            destination_id,
            reply: ReplyRouter::from_registry(),
            marker: PhantomData,
        }
    }

    pub fn into_endpoint<T: PublicMessage>(self) -> RemoteEndpoint<T> {
        RemoteEndpoint {
            node_id: self.0,
            destination_id: public_destination(T::ID),
            reply: ReplyRouter::from_registry(),
            marker: PhantomData,
        }
    }
}

pub struct RemoteEndpoint<T> {
    node_id: NodeId,
    destination_id: DestinationId,
    reply: actix::Addr<ReplyRouter>,
    marker: PhantomData<T>,
}

impl<T> RemoteEndpoint<T>
where
    T: Message + Send + Serialize + 'static,
    <T as Message>::Result: Send + DeserializeOwned,
{
    pub fn send(&self, msg: T) -> impl Future<Item = T::Result, Error = reply::SendError> {
        self.reply
            .send(CallRemote(self.node_id, self.destination_id.clone(), msg))
            .flatten_fut()
    }
}
