use super::error;
use actix::prelude::*;
use futures::prelude::*;

use super::util::*;
use rand::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json;
use smallvec::*;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Write};

pub type NodeId = [u8; 32];
pub type MessageId = SmallVec<[u8; 8]>;
pub type DestinationId = SmallVec<[u8; 8]>;
pub type MessageTypeId = SmallVec<[u8; 4]>;

pub struct IdGenerator<R: Rng> {
    rng: R,
    state: [u64; 1],
}

impl<R: Rng> IdGenerator<R> {
    fn new(mut r: R) -> IdGenerator<R> {
        let mut s = IdGenerator {
            state: r.gen(),
            rng: r,
        };

        s
    }

    fn next_dest(&mut self) -> DestinationId {
        use std::mem;

        self.state[0] = self.state[0].wrapping_add(1);

        let bytes: [u8; 8] = unsafe { mem::transmute::<[u64; 1], [u8; 8]>(self.state) };
        SmallVec::from_slice(&bytes[..])
    }
}

thread_local! {
    static ID_GEN : RefCell<IdGenerator<ThreadRng>> = RefCell::new(IdGenerator::new(thread_rng()));
}

pub fn gen_destination_id() -> DestinationId {
    ID_GEN.with(|gen| gen.borrow_mut().next_dest())
}

pub fn public_destination(destination_id: u32) -> DestinationId {
    use byteorder::{BigEndian, WriteBytesExt};
    let mut v = SmallVec::new();

    v.write_u32::<BigEndian>(0xdeadbeef);
    v.write_u32::<BigEndian>(destination_id);

    v
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TransportError {
    NoDestination,
    BadFormat(String),
}

impl Into<error::Error> for TransportError {
    fn into(self) -> error::Error {
        match self {
            TransportError::NoDestination => error::ErrorKind::NoDestination.into(),
            TransportError::BadFormat(s) => error::ErrorKind::BadFormat(s).into(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TransportResult<B> {
    Ok(B),
    Err(TransportError),
}

impl<B> TransportResult<B> {
    pub const NoDestination: Self = TransportResult::Err(TransportError::NoDestination);

    pub fn bad_request<T: Into<String>>(msg: T) -> Self {
        TransportResult::Err(TransportError::BadFormat(msg.into()))
    }
}

impl<T> Into<Result<T, error::Error>> for TransportResult<T> {
    fn into(self) -> Result<T, error::Error> {
        match self {
            TransportResult::Ok(t) => Ok(t),
            TransportResult::Err(e) => Err(e.into()),
        }
    }
}

impl<B: Default> Default for TransportResult<B> {
    fn default() -> Self {
        TransportResult::Ok(B::default())
    }
}

#[derive(Message, Clone)]
pub struct RouteMessage<B> {
    pub msg_id: MessageId,
    pub sender: NodeId,
    pub destination: DestinationId,
    pub reply_to: Option<DestinationId>,
    pub correlation_id: Option<MessageId>,
    pub ts: u64,
    pub expires: Option<u64>,
    pub body: B,
}

impl<B> RouteMessage<B> {
    pub fn do_reply<T, F: FnOnce(EmitMessage<T>)>(&self, arg: T, f: F) {
        if let Some(msg) = EmitMessage::reply(self, TransportResult::Ok(arg)) {
            f(msg)
        }
    }

    pub fn unit(&self) -> RouteMessage<()> {
        RouteMessage {
            msg_id: self.msg_id.clone(),
            sender: self.sender.clone(),
            destination: self.destination.clone(),
            reply_to: self.reply_to.clone(),
            correlation_id: self.correlation_id.clone(),
            ts: self.ts,
            expires: self.expires,
            body: (),
        }
    }
}

impl RouteMessage<String> {
    pub fn from_json<B: DeserializeOwned>(self) -> serde_json::Result<RouteMessage<B>> {
        let body: B = serde_json::from_str(self.body.as_ref())?;
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

impl<B: Serialize> RouteMessage<B> {
    fn json(self) -> serde_json::Result<EmitMessage<String>> {
        unimplemented!()
    }
}

#[derive(Default)]
pub struct EmitMessage<B> {
    pub dest_node: NodeId,
    pub destination: DestinationId,
    pub correlation_id: Option<MessageId>,
    pub ts: u64,
    pub expires: Option<u64>,
    pub body: TransportResult<B>,
}

impl<B: BinPack> BinPack for TransportResult<B> {
    fn pack_to_stream<W: io::Write + ?Sized>(&self, w: &mut W) -> io::Result<()> {
        use byteorder::WriteBytesExt;

        match self {
            TransportResult::Ok(b) => {
                w.write_u8(0)?;
                b.pack_to_stream(w)
            }
            TransportResult::Err(e) => {
                let err_code = match e {
                    TransportError::NoDestination => 10u8,
                    TransportError::BadFormat(_s) => 11u8,
                };
                w.write_u8(err_code)
            }
        }
    }

    fn unpack_from_stream<R: io::Read + ?Sized>(&mut self, r: &mut R) -> io::Result<()> {
        unimplemented!()
    }
}

impl<B> EmitMessage<B> {
    pub fn reply<BX>(msg: &RouteMessage<BX>, body: TransportResult<B>) -> Option<Self> {
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
        let body = match self.body {
            TransportResult::Ok(ref b) => TransportResult::Ok(serde_json::to_string(b)?),
            TransportResult::Err(e) => TransportResult::Err(e),
        };

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

impl EmitMessage<String> {
    fn to_vec(self, msg_id: &MessageId) -> io::Result<Vec<u8>> {
        //use std::io::Write;

        let mut buf: Vec<u8> = Vec::new();

        buf.pack(msg_id)?;
        buf.pack(self.dest_node.as_ref())?;
        buf.pack(&self.destination)?;
        buf.pack(&self.correlation_id)?;
        buf.pack(&self.ts)?;
        buf.pack(&self.expires)?;
        buf.pack(&self.body)?;
        Ok(buf)
    }

    fn from_vec(buf: &[u8]) -> io::Result<(MessageId, Self)> {
        let mut c = io::Cursor::new(buf);

        let mut msg: EmitMessage<String> = EmitMessage::default();

        let mut msg_id = MessageId::default();

        c.unpack(&mut msg_id)?;
        c.unpack(msg.dest_node.as_mut())?;
        c.unpack(&mut msg.destination)?;
        c.unpack(&mut msg.correlation_id)?;
        c.unpack(&mut msg.ts)?;
        c.unpack(&mut msg.expires)?;
        c.unpack(&mut msg.body)?;

        Ok((msg_id, msg))
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
// 4. Przekazujemy do odbiorcy i na odpowiedzi serializujemy i pchamy do endponitu na pole
//    destination = msg.reply_to
//    correlation_id = msg.msg_id

// Schemat nadawania
// 1. Tworzymy nowy komunikat dostajemy msg_id
// 2. rejestrujemy odbiorcę w reply_handler (ustawiamy timeout)
// 3. serializujemy i nadajemy.
//
