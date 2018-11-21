use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use smallvec;
use std::{
    io::{self, Read, Write},
    u16, u32, u64,
};

pub trait Narrow<T> {
    fn narrow(self) -> Option<T>;
}

impl Narrow<u16> for usize {
    #[inline]
    fn narrow(self) -> Option<u16> {
        if self > u16::MAX as usize {
            None
        } else {
            Some(self as u16)
        }
    }
}

impl Narrow<u32> for usize {
    #[inline]
    fn narrow(self) -> Option<u32> {
        if self > u16::MAX as usize {
            None
        } else {
            Some(self as u32)
        }
    }
}
