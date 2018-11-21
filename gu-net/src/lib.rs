#![allow(unused)]

extern crate actix;
extern crate actix_web;
extern crate futures;
extern crate quick_protobuf;
extern crate smallvec;
extern crate tokio_io;
#[macro_use]
extern crate log;

#[macro_use]
extern crate error_chain;

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

extern crate byteorder;
extern crate gu_actix;
extern crate rand;

use futures::{future, stream};
use tokio_io::{AsyncRead, AsyncWrite, IoStream};

const PROTO_RPC: &[u8] = &[0x8Cu8, 0xC3, 0x34, 0xBE];

pub type NodeId = types::NodeId;

type Key = [u8; 20];

mod node_info;
mod proto;
pub mod rpc;
pub mod types;
