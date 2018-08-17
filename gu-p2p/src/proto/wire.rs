//! Automatically generated rust module for 'wire.proto' file

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(unknown_lints)]
#![allow(clippy)]
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

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Hello<'a> {
    pub role: Role,
    pub node_name: Cow<'a, str>,
    pub node_id: Cow<'a, [u8]>,
    pub version: Vec<u32>,
}

impl<'a> MessageRead<'a> for Hello<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(8) => msg.role = r.read_enum(bytes)?,
                Ok(18) => msg.node_name = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(26) => msg.node_id = r.read_bytes(bytes).map(Cow::Borrowed)?,
                Ok(32) => msg.version.push(r.read_uint32(bytes)?),
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
        + 1 + sizeof_len((&self.node_name).len())
        + 1 + sizeof_len((&self.node_id).len())
        + self.version.iter().map(|s| 1 + sizeof_varint(*(s) as u64)).sum::<usize>()
    }

    fn write_message<W: Write>(&self, w: &mut Writer<W>) -> Result<()> {
        w.write_with_tag(8, |w| w.write_enum(*&self.role as i32))?;
        w.write_with_tag(18, |w| w.write_string(&**&self.node_name))?;
        w.write_with_tag(26, |w| w.write_bytes(&**&self.node_id))?;
        for s in &self.version { w.write_with_tag(32, |w| w.write_uint32(*s))?; }
        Ok(())
    }
}

