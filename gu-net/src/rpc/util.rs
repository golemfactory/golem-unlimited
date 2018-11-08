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

pub trait BinPack {
    fn pack_to_stream<W: io::Write + ?Sized>(&self, w: &mut W) -> io::Result<()>;

    fn unpack_from_stream<R: io::Read + ?Sized>(&mut self, r: &mut R) -> io::Result<()>;
}

pub trait WriteExt {
    fn pack<P: BinPack + ?Sized>(&mut self, data: &P) -> io::Result<()>;
}

pub trait ReadExt {
    fn unpack<P: BinPack + ?Sized>(&mut self, data: &mut P) -> io::Result<()>;
}

impl<W: io::Write> WriteExt for W {
    #[inline]
    fn pack<P: BinPack + ?Sized>(&mut self, data: &P) -> io::Result<()> {
        data.pack_to_stream(self)
    }
}

impl<R: io::Read> ReadExt for R {
    fn unpack<P: BinPack + ?Sized>(&mut self, data: &mut P) -> io::Result<()> {
        data.unpack_from_stream(self)
    }
}

impl<A: smallvec::Array<Item = u8>> BinPack for smallvec::SmallVec<A> {
    fn pack_to_stream<W: io::Write + ?Sized>(&self, w: &mut W) -> io::Result<()> {
        w.write_u16::<BigEndian>(self.len().narrow().unwrap())?;
        w.write_all(self.as_ref())?;
        Ok(())
    }

    fn unpack_from_stream<R: io::Read + ?Sized>(&mut self, r: &mut R) -> io::Result<()> {
        let v = r.read_u16::<BigEndian>()?;
        self.resize(v as usize, 0);
        r.read_exact(self.as_mut_slice())?;
        Ok(())
    }
}

impl<A: smallvec::Array<Item = u8>> BinPack for Option<smallvec::SmallVec<A>> {
    fn pack_to_stream<W: io::Write + ?Sized>(&self, w: &mut W) -> io::Result<()> {
        match self {
            Some(b) => {
                w.write_u16::<BigEndian>(b.len().narrow().unwrap())?;
                w.write_all(b.as_ref())
            }
            None => w.write_u16::<BigEndian>(0),
        }
    }

    fn unpack_from_stream<R: io::Read + ?Sized>(&mut self, r: &mut R) -> io::Result<()> {
        let v = r.read_u16::<BigEndian>()?;
        if v == 0 {
            *self = None
        } else {
            let mut vx = smallvec::SmallVec::new();
            vx.resize(v as usize, 0);
            r.read_exact(vx.as_mut_slice())?;
            *self = Some(vx);
        }
        Ok(())
    }
}

impl BinPack for [u8] {
    fn pack_to_stream<W: io::Write + ?Sized>(&self, w: &mut W) -> io::Result<()> {
        w.write_all(self)
    }

    fn unpack_from_stream<R: io::Read + ?Sized>(&mut self, r: &mut R) -> io::Result<()> {
        r.read_exact(self)
    }
}

impl BinPack for u64 {
    fn pack_to_stream<W: io::Write + ?Sized>(&self, w: &mut W) -> io::Result<()> {
        w.write_u64::<BigEndian>(*self)
    }

    fn unpack_from_stream<R: io::Read + ?Sized>(&mut self, r: &mut R) -> io::Result<()> {
        *self = r.read_u64::<BigEndian>()?;
        Ok(())
    }
}

impl BinPack for Option<u64> {
    fn pack_to_stream<W: io::Write + ?Sized>(&self, w: &mut W) -> io::Result<()> {
        match self {
            None => w.write_u64::<BigEndian>(u64::MAX),
            Some(v) => w.write_u64::<BigEndian>(*v),
        }
    }

    fn unpack_from_stream<R: io::Read + ?Sized>(&mut self, r: &mut R) -> io::Result<()> {
        let v = r.read_u64::<BigEndian>()?;
        if v == u64::MAX {
            *self = None
        } else {
            *self = Some(v)
        }
        Ok(())
    }
}

impl BinPack for String {
    fn pack_to_stream<W: io::Write + ?Sized>(&self, w: &mut W) -> io::Result<()> {
        w.write_u32::<BigEndian>(self.len().narrow().unwrap())?;
        w.write_all(self.as_ref())?;
        Ok(())
    }

    fn unpack_from_stream<R: io::Read + ?Sized>(&mut self, r: &mut R) -> io::Result<()> {
        let v = r.read_u32::<BigEndian>()?;

        let mut vx: Vec<u8> = Vec::with_capacity(v as usize);
        r.read_exact(vx.as_mut_slice())?;
        *self = String::from_utf8(vx).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(())
    }
}
