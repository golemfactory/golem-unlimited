extern crate futures;
extern crate tokio_io;
extern crate quick_protobuf;

use futures::{future, stream};
use tokio_io::{AsyncRead, AsyncWrite, IoStream};

const PROTO_RPC: &[u8] = &[0x8Cu8, 0xC3, 0x34, 0xBE];

type NodeId = [u64; 4];

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