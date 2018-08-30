extern crate actix;
extern crate actix_web;
extern crate futures;
extern crate quick_protobuf;
extern crate smallvec;
extern crate tokio_io;

#[macro_use]
extern crate error_chain;

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

extern crate byteorder;

use futures::{future, stream};
use tokio_io::{AsyncRead, AsyncWrite, IoStream};

const PROTO_RPC: &[u8] = &[0x8Cu8, 0xC3, 0x34, 0xBE];

type NodeId = [u8; 32];

type Key = [u8; 20];

type ConnectionRef = Box<Connection>;

pub struct ProtoSpec;

struct Error;

trait Network {
    type ConnectionFuture: future::Future<Item = ConnectionRef, Error = Error>;

    fn connect(&mut self, peer: NodeId, proto: ProtoSpec) -> Self::ConnectionFuture;

    fn listen(
        &mut self,
        proto: ProtoSpec,
    ) -> Box<stream::Stream<Item = ConnectionRef, Error = Error>>;
}

trait Discovery {}

trait Connection: AsyncRead + AsyncWrite {
    fn peer(&self) -> NodeId;
}

mod proto;
mod rpc;
