
use futures::prelude::*;
use futures::future;

use std::io;
use std::cmp::min;
use bytes::Bytes;
use tar::Archive;

pub struct FakeBuffer {
    bytes : Vec<u8>,
    eof: bool
}

impl FakeBuffer {
    fn append(&mut self, bytes : &Bytes) {
        self.bytes.extend_from_slice(bytes.as_ref())
    }

    fn close(&mut self) {
        self.eof = true;
    }
}

impl io::Read for FakeBuffer {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let len = if self.eof {
            min(self.bytes.len(), buf.len())
        } else {
            buf.len()
        };

        if len == 0 {
            return Ok(0)
        }

        if self.bytes.len() <= buf.len() {
            buf[0..len].copy_from_slice(&self.bytes[0..len]);
            let _ = self.bytes.drain(0..len);
            Ok(len)
        }
        else {
            Err(io::ErrorKind::WouldBlock.into())
        }
    }
}

pub struct EntryStream<T> {
    upstream  : T,
    archive : Archive<FakeBuffer>,
}

impl<'a, T : Stream<Item=Bytes>> Stream for &'a mut EntryStream<T> {
    type Item = tar::Entry<'a, FakeBuffer>;
    type Error = ();

    fn poll(&mut self) -> Result<Async<Option<<Self as Stream>::Item>>, <Self as Stream>::Error> {
        unimplemented!()
    }
}