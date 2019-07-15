//! Automatically generated rust module for 'wire.proto' file

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(unknown_lints)]
#![allow(clippy::all)]
#![cfg_attr(rustfmt, rustfmt_skip)]


use std::io::Write;
use std::borrow::Cow;
use quick_protobuf::{MessageRead, MessageWrite, BytesReader, Writer, Result};
use quick_protobuf::sizeofs::*;
use super::*;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Role {
    HUB = 1,
    PROVIDER = 2,
    BOTH = 3,
}

impl Default for Role {
    fn default() -> Self {
        Role::HUB
    }
}

impl From<i32> for Role {
    fn from(i: i32) -> Self {
        match i {
            1 => Role::HUB,
            2 => Role::PROVIDER,
            3 => Role::BOTH,
            _ => Self::default(),
        }
    }
}

impl<'a> From<&'a str> for Role {
    fn from(s: &'a str) -> Self {
        match s {
            "HUB" => Role::HUB,
            "PROVIDER" => Role::PROVIDER,
            "BOTH" => Role::BOTH,
            _ => Self::default(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RpcStatus {
    Request = 0,
    Reply = 1,
    Event = 2,
    NoDestination = 100,
    BadFormat = 101,
}

impl Default for RpcStatus {
    fn default() -> Self {
        RpcStatus::Request
    }
}

impl From<i32> for RpcStatus {
    fn from(i: i32) -> Self {
        match i {
            0 => RpcStatus::Request,
            1 => RpcStatus::Reply,
            2 => RpcStatus::Event,
            100 => RpcStatus::NoDestination,
            101 => RpcStatus::BadFormat,
            _ => Self::default(),
        }
    }
}

impl<'a> From<&'a str> for RpcStatus {
    fn from(s: &'a str) -> Self {
        match s {
            "Request" => RpcStatus::Request,
            "Reply" => RpcStatus::Reply,
            "Event" => RpcStatus::Event,
            "NoDestination" => RpcStatus::NoDestination,
            "BadFormat" => RpcStatus::BadFormat,
            _ => Self::default(),
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Hello<'a> {
    pub role: Role,
    pub node_name: Option<Cow<'a, str>>,
    pub node_id: Cow<'a, [u8]>,
    pub instance_id: Cow<'a, [u8]>,
    pub version: Option<Cow<'a, str>>,
    pub os: Option<Cow<'a, str>>,
    pub max_ram: Option<u64>,
    pub max_storage: Option<u64>,
    pub exec_envs: Vec<Cow<'a, str>>,
}

impl<'a> MessageRead<'a> for Hello<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(8) => msg.role = r.read_enum(bytes)?,
                Ok(18) => msg.node_name = Some(r.read_string(bytes).map(Cow::Borrowed)?),
                Ok(26) => msg.node_id = r.read_bytes(bytes).map(Cow::Borrowed)?,
                Ok(34) => msg.instance_id = r.read_bytes(bytes).map(Cow::Borrowed)?,
                Ok(42) => msg.version = Some(r.read_string(bytes).map(Cow::Borrowed)?),
                Ok(82) => msg.os = Some(r.read_string(bytes).map(Cow::Borrowed)?),
                Ok(88) => msg.max_ram = Some(r.read_uint64(bytes)?),
                Ok(96) => msg.max_storage = Some(r.read_uint64(bytes)?),
                Ok(106) => msg.exec_envs.push(r.read_string(bytes).map(Cow::Borrowed)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Hello<'a> {
    fn get_size(&self) -> usize {
        0
        + 1 + sizeof_varint(*(&self.role) as u64)
        + self.node_name.as_ref().map_or(0, |m| 1 + sizeof_len((m).len()))
        + 1 + sizeof_len((&self.node_id).len())
        + 1 + sizeof_len((&self.instance_id).len())
        + self.version.as_ref().map_or(0, |m| 1 + sizeof_len((m).len()))
        + self.os.as_ref().map_or(0, |m| 1 + sizeof_len((m).len()))
        + self.max_ram.as_ref().map_or(0, |m| 1 + sizeof_varint(*(m) as u64))
        + self.max_storage.as_ref().map_or(0, |m| 1 + sizeof_varint(*(m) as u64))
        + self.exec_envs.iter().map(|s| 1 + sizeof_len((s).len())).sum::<usize>()
    }

    fn write_message<W: Write>(&self, w: &mut Writer<W>) -> Result<()> {
        w.write_with_tag(8, |w| w.write_enum(*&self.role as i32))?;
        if let Some(ref s) = self.node_name { w.write_with_tag(18, |w| w.write_string(&**s))?; }
        w.write_with_tag(26, |w| w.write_bytes(&**&self.node_id))?;
        w.write_with_tag(34, |w| w.write_bytes(&**&self.instance_id))?;
        if let Some(ref s) = self.version { w.write_with_tag(42, |w| w.write_string(&**s))?; }
        if let Some(ref s) = self.os { w.write_with_tag(82, |w| w.write_string(&**s))?; }
        if let Some(ref s) = self.max_ram { w.write_with_tag(88, |w| w.write_uint64(*s))?; }
        if let Some(ref s) = self.max_storage { w.write_with_tag(96, |w| w.write_uint64(*s))?; }
        for s in &self.exec_envs { w.write_with_tag(106, |w| w.write_string(&**s))?; }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct HelloReply<'a> {
    pub role: Role,
    pub node_name: Option<Cow<'a, str>>,
    pub node_id: Cow<'a, [u8]>,
    pub version: Option<Cow<'a, str>>,
    pub max_ping_ms: Option<i32>,
}

impl<'a> MessageRead<'a> for HelloReply<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(8) => msg.role = r.read_enum(bytes)?,
                Ok(18) => msg.node_name = Some(r.read_string(bytes).map(Cow::Borrowed)?),
                Ok(26) => msg.node_id = r.read_bytes(bytes).map(Cow::Borrowed)?,
                Ok(34) => msg.version = Some(r.read_string(bytes).map(Cow::Borrowed)?),
                Ok(160) => msg.max_ping_ms = Some(r.read_int32(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for HelloReply<'a> {
    fn get_size(&self) -> usize {
        0
        + 1 + sizeof_varint(*(&self.role) as u64)
        + self.node_name.as_ref().map_or(0, |m| 1 + sizeof_len((m).len()))
        + 1 + sizeof_len((&self.node_id).len())
        + self.version.as_ref().map_or(0, |m| 1 + sizeof_len((m).len()))
        + self.max_ping_ms.as_ref().map_or(0, |m| 2 + sizeof_varint(*(m) as u64))
    }

    fn write_message<W: Write>(&self, w: &mut Writer<W>) -> Result<()> {
        w.write_with_tag(8, |w| w.write_enum(*&self.role as i32))?;
        if let Some(ref s) = self.node_name { w.write_with_tag(18, |w| w.write_string(&**s))?; }
        w.write_with_tag(26, |w| w.write_bytes(&**&self.node_id))?;
        if let Some(ref s) = self.version { w.write_with_tag(34, |w| w.write_string(&**s))?; }
        if let Some(ref s) = self.max_ping_ms { w.write_with_tag(160, |w| w.write_int32(*s))?; }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct RpcMessage<'a> {
    pub message_id: Cow<'a, [u8]>,
    pub destination_id: Cow<'a, [u8]>,
    pub reply_to: Option<Cow<'a, [u8]>>,
    pub correlation_id: Option<Cow<'a, [u8]>>,
    pub ts: Option<u64>,
    pub expires: Option<u64>,
    pub status: RpcStatus,
    pub payload: Option<Cow<'a, str>>,
}

impl<'a> MessageRead<'a> for RpcMessage<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.message_id = r.read_bytes(bytes).map(Cow::Borrowed)?,
                Ok(18) => msg.destination_id = r.read_bytes(bytes).map(Cow::Borrowed)?,
                Ok(42) => msg.reply_to = Some(r.read_bytes(bytes).map(Cow::Borrowed)?),
                Ok(26) => msg.correlation_id = Some(r.read_bytes(bytes).map(Cow::Borrowed)?),
                Ok(80) => msg.ts = Some(r.read_uint64(bytes)?),
                Ok(88) => msg.expires = Some(r.read_uint64(bytes)?),
                Ok(32) => msg.status = r.read_enum(bytes)?,
                Ok(162) => msg.payload = Some(r.read_string(bytes).map(Cow::Borrowed)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for RpcMessage<'a> {
    fn get_size(&self) -> usize {
        0
        + 1 + sizeof_len((&self.message_id).len())
        + 1 + sizeof_len((&self.destination_id).len())
        + self.reply_to.as_ref().map_or(0, |m| 1 + sizeof_len((m).len()))
        + self.correlation_id.as_ref().map_or(0, |m| 1 + sizeof_len((m).len()))
        + self.ts.as_ref().map_or(0, |m| 1 + sizeof_varint(*(m) as u64))
        + self.expires.as_ref().map_or(0, |m| 1 + sizeof_varint(*(m) as u64))
        + 1 + sizeof_varint(*(&self.status) as u64)
        + self.payload.as_ref().map_or(0, |m| 2 + sizeof_len((m).len()))
    }

    fn write_message<W: Write>(&self, w: &mut Writer<W>) -> Result<()> {
        w.write_with_tag(10, |w| w.write_bytes(&**&self.message_id))?;
        w.write_with_tag(18, |w| w.write_bytes(&**&self.destination_id))?;
        if let Some(ref s) = self.reply_to { w.write_with_tag(42, |w| w.write_bytes(&**s))?; }
        if let Some(ref s) = self.correlation_id { w.write_with_tag(26, |w| w.write_bytes(&**s))?; }
        if let Some(ref s) = self.ts { w.write_with_tag(80, |w| w.write_uint64(*s))?; }
        if let Some(ref s) = self.expires { w.write_with_tag(88, |w| w.write_uint64(*s))?; }
        w.write_with_tag(32, |w| w.write_enum(*&self.status as i32))?;
        if let Some(ref s) = self.payload { w.write_with_tag(162, |w| w.write_string(&**s))?; }
        Ok(())
    }
}

