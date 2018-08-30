use super::error;
use actix::prelude::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json;
use smallvec::*;
use std::any::Any;
use std::collections::HashMap;

type NodeId = [u8; 32];
type MessageId = SmallVec<[u8; 8]>;
type DestinationId = SmallVec<[u8; 8]>;
type MessageTypeId = SmallVec<[u8; 4]>;

#[derive(Serialize, Deserialize)]
enum TransportError {
    NoDestination,
}

#[derive(Message)]
struct RouteMessage<B> {
    msg_id: MessageId,
    sender: NodeId,
    destination: DestinationId,
    reply_to: Option<DestinationId>,
    correlation_id: Option<MessageId>,
    ts: u64,
    expires: Option<u64>,
    body: B,
}

impl RouteMessage<String> {
    fn from_json<B : DeserializeOwned>(self) -> serde_json::Result<RouteMessage<B>> {
        let body : B = serde_json::from_str(self.body.as_ref())?;
        Ok(RouteMessage {
            msg_id: self.msg_id,
            sender: self.sender,
            destination: self.destination,
            reply_to: self.reply_to,
            correlation_id: self.correlation_id,
            ts: self.ts,
            expires: self.expires,
            body,
        })
    }
}

impl<B : Serialize> RouteMessage<B> {
    fn json(self) ->  serde_json::Result<EmitMessage<String>> {
        unimplemented!()
    }
}

struct EmitMessage<B> {
    dest_node: NodeId,
    destination: DestinationId,
    correlation_id: Option<MessageId>,
    ts: u64,
    expires: Option<u64>,
    body: B,
}

impl<B> EmitMessage<B> {
    fn reply<BX>(msg: &RouteMessage<BX>, body: B) -> Option<Self> {
        match msg.reply_to.clone() {
            Some(reply_to) => Some(EmitMessage {
                dest_node: msg.sender.clone(),
                destination: reply_to,
                correlation_id: Some(
                    msg.correlation_id
                        .clone()
                        .unwrap_or_else(|| msg.msg_id.clone()),
                ),
                ts: 0,
                expires: msg.expires.clone(),
                body,
            }),
            None => None,
        }
    }
}

impl<B: Serialize> EmitMessage<B> {
    fn json(self) -> serde_json::Result<EmitMessage<String>> {
        let body = serde_json::to_string(&self.body)?;

        Ok(EmitMessage {
            dest_node: self.dest_node,
            destination: self.destination,
            correlation_id: self.correlation_id,
            ts: self.ts,
            expires: self.expires,
            body,
        })
    }
}

trait Narrow<T> {

    fn narrow(self) -> Option<T>;
}

impl Narrow<u16> for usize {

    #[inline]
    fn narrow(self) -> Option<u16> {
        if self > std::u16::MAX as usize {
            None
        }
        else {
            Some(self as u16)
        }
    }
}

impl EmitMessage<String> {

    fn to_vec(self, msg_id : &MessageId) -> Vec<u8> {
        use std::io::Write;
        use byteorder::{BigEndian, WriteBytesExt};

        let mut buf = Vec::new();
        buf.write_u16(msg_id.len().narrow().unwrap());
        buf.write(msg_id.as_ref());
        

        buf
    }

}

impl<B> Message for EmitMessage<B> {
    type Result = Result<MessageId, error::Error>;
}

// Schemat odbierania
//
// 1. Wiadomość przychodzi z Endpointu
// 2. Wyszukiwany jest odbiorca, gdy brak NoDestinationFound
// 3. Deserializujemy komunikat, gdy bład WireFormatError
// 4. Przekazujemy do odbiorcy i na odpowiedzi serializujemy i phamy do endponitu na pole
//    destination = msg.reply_to
//    correlation_id = msg.msg_id

// Shemat nadawania
// 1. Tworzymy nowy komunikat dostajemy msg_id
// 2. rejestrujemy odbiorce w reply_handler (ustawiamy timeout)
// 3. serializujemy i nadajemy.
//

trait LocalEndpoint {
    fn handle(
        &mut self,
        message: RouteMessage<String>,
        ctx: &mut <MessageRouter as Actor>::Context,
    );
}

pub struct MessageRouter {
    destinations: HashMap<DestinationId, Box<LocalEndpoint + 'static>>,
    remotes: HashMap<NodeId, Recipient<EmitMessage<Result<String, TransportError>>>>,
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

impl Handler<RouteMessage<String>> for MessageRouter {
    type Result = ();

    fn handle(&mut self, msg: RouteMessage<String>, ctx: &mut Self::Context) -> Self::Result {
        //let destination = msg.destination.clone();
        if let Some(v) = self.destinations.get_mut(&msg.destination) {
            v.handle(msg, ctx);
        }
        ()
    }
}
