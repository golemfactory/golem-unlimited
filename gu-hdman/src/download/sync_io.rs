use super::Error;
use futures::prelude::*;
use futures::sync::oneshot;
use futures_cpupool::CpuPool;
use gu_actix::safe::*;
use serde::{Deserialize, Serialize};
use std::io::prelude::*;
use std::{fs, io, path};

use std::cell::RefCell;
use std::sync::Arc;

pub(super) struct DownloadFile {
    temp_file_name: path::PathBuf,
    inner: fs::File,
    meta: LogMetadata,
    crc_map: Vec<u64>,
    map_offset: u64,
}

struct Chunk {
    from: u64,
    to: u64,
}

const MAGIC: [u8; 8] = [0xf4, 0xd4, 0xc7, 0xd1, 0x4d, 0x2f, 0xe2, 0x83];
const CHUNK_SIZE: u64 = 1024 * 512;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub(super) enum CheckType {
    ETag(String),
    ModTime(String),
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(super) struct LogMetadata {
    file_name: String,
    pub(super) url: String,
    pub(super) check: CheckType,
    pub(super) size: u64,
    pub(super) chunks: u32,
    chunk_size: u32,
    ts: chrono::DateTime<chrono::Utc>,
}

impl LogMetadata {
    pub fn to_if_range(&self) -> Option<&str> {
        match &self.check {
            CheckType::ETag(etag) => Some(etag.as_ref()),
            CheckType::ModTime(mod_time) => Some(mod_time.as_ref()),
            CheckType::None => None,
        }
    }
}

// File Structure
//
// [...data....][MAGIC][LogMetadata][crc64][chunk_crc64_0][chunk_crc64_1]...[chunk_crc64_<n>][other data][offset : 64][offset_binmap : 64]
//
//

fn read_u64<R: io::Read>(f: &mut R) -> io::Result<u64> {
    let mut u64_bytes = [0u8; 8];
    f.read_exact(&mut u64_bytes)?;
    Ok(u64::from_le_bytes(u64_bytes))
}

fn write_u64<W: io::Write>(w: &mut W, v: u64) -> io::Result<()> {
    let mut bytes = v.to_le_bytes();

    w.write_all(bytes.as_mut())
}

fn recover_file(
    download_file_name: &path::Path,
    meta: &LogMetadata,
) -> Result<DownloadFile, Error> {
    let mut part_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(download_file_name)?;
    part_file.seek(io::SeekFrom::End(-16))?;

    let head_offset = read_u64(&mut part_file)?;
    let map_offset = read_u64(&mut part_file)?;

    if map_offset <= head_offset {
        return Err(Error::InvalidTrackingFile("invalid metadata offset"));
    }

    let mut buf = [0u8; 8];
    part_file.seek(io::SeekFrom::Start(head_offset))?;
    part_file.read_exact(&mut buf)?;
    if buf != MAGIC {
        return Err(Error::InvalidTrackingFile("bad magic code"));
    }
    let size = (map_offset - head_offset - 16).cast_into()?;
    if size > 0x1_000_000 {
        // 16MB
        return Err(Error::InvalidTrackingFile("overflow metadata size"));
    }

    let mut buf = Vec::with_capacity(size);
    buf.resize(size, 0);
    part_file.read_exact(buf.as_mut())?;

    let crc64 = read_u64(&mut part_file)?;

    let computed_crc64 = crc::crc64::checksum_iso(buf.as_slice());

    if crc64 != computed_crc64 {
        return Err(Error::InvalidTrackingFile("checksum fail"));
    }

    let file_meta: LogMetadata = bincode::deserialize(buf.as_ref())?;

    if file_meta.url != meta.url || file_meta.size != meta.size || file_meta.check != meta.check {
        return Err(Error::InvalidTrackingFile("metadata changed"));
    }

    part_file.seek(io::SeekFrom::Start(map_offset))?;
    let chunks = file_meta.chunks;
    let mut crc_map = Vec::with_capacity(chunks.cast_into()?);

    for _ in 0..chunks {
        let chunk_crc64 = read_u64(&mut part_file)?;
        crc_map.push(chunk_crc64);
    }

    Ok(DownloadFile {
        temp_file_name: download_file_name.into(),
        inner: part_file,
        meta: file_meta,
        map_offset,
        crc_map,
    })
}

fn new_part_file(
    download_file_name: &path::Path,
    meta: LogMetadata,
) -> Result<DownloadFile, Error> {
    let mut part_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(download_file_name)?;

    let meta_bytes = bincode::serialize(&meta)?;

    let head_offset = meta.size;
    let map_offset = head_offset + 8 + meta_bytes.len() as u64 + 8;
    let tail_offset = map_offset + meta.chunks as u64 * 8u64;
    let total_file_size = tail_offset + 8 + 8;

    part_file.set_len(total_file_size)?;
    part_file.seek(io::SeekFrom::Start(head_offset))?;
    part_file.write_all(MAGIC.as_ref())?;
    part_file.write_all(meta_bytes.as_ref())?;
    let computed_crc64 = crc::crc64::checksum_iso(meta_bytes.as_ref());
    write_u64(&mut part_file, computed_crc64)?;

    // At map offset
    debug_assert_eq!(part_file.seek(io::SeekFrom::Current(0))?, map_offset);
    let crc_map = (0..meta.chunks).map(|_| 0u64).collect();

    for _ in 0..meta.chunks {
        write_u64(&mut part_file, 0)?;
    }
    debug_assert_eq!(part_file.seek(io::SeekFrom::Current(0))?, tail_offset);
    write_u64(&mut part_file, head_offset)?;
    write_u64(&mut part_file, map_offset)?;

    Ok(DownloadFile {
        temp_file_name: download_file_name.into(),
        inner: part_file,
        meta,
        crc_map,
        map_offset,
    })
}

fn chunk_crc64(bytes: &[u8]) -> u64 {
    let crc64 = crc::crc64::checksum_iso(bytes);

    if crc64 == 0 {
        ::std::u64::MAX
    } else {
        crc64
    }
}

impl DownloadFile {
    pub fn new<'a>(
        file_name: &'a str,
        url: &'a str,
        check: CheckType,
        size: u64,
    ) -> Result<DownloadFile, Error> {
        let chunk_size = CHUNK_SIZE as u32;
        let chunks: u32 = ((size + (CHUNK_SIZE as u64) - 1) / CHUNK_SIZE).cast_into()?;
        let download_file_name: path::PathBuf = format!("{}.gu-download", file_name).into();
        let ts = chrono::Utc::now();
        let meta = LogMetadata {
            file_name: file_name.into(),
            url: url.into(),
            check,
            size,
            chunks,
            chunk_size,
            ts,
        };

        if download_file_name.exists() {
            match recover_file(&download_file_name, &meta) {
                Err(Error::InvalidTrackingFile(err_message)) => {
                    log::warn!("recovery part file problem: {}", err_message);
                    ()
                }
                result => return result,
            }
        }
        new_part_file(&download_file_name, meta)
    }

    pub fn chunk(&self, chunk_nr: u32) -> (u64, u64) {
        if chunk_nr == self.meta.chunks - 1 {
            (
                chunk_nr as u64 * self.meta.chunk_size as u64,
                self.meta.size,
            )
        } else {
            let from = chunk_nr as u64 * self.meta.chunk_size as u64;
            (from, from + self.meta.chunk_size as u64)
        }
    }

    pub fn meta(&self) -> LogMetadata {
        self.meta.clone()
    }

    pub fn add_chunk(&mut self, from: u64, to: u64, bytes: &[u8]) -> Result<(), Error> {
        let chunk_nr = from / self.meta.chunk_size as u64;
        assert_eq!(bytes.len() as u64, to - from);
        assert_eq!(self.chunk(chunk_nr.cast_into()?), (from, to));

        self.inner.seek(io::SeekFrom::Start(from))?;
        self.inner.write_all(bytes)?;
        let crc64 = chunk_crc64(bytes);
        self.inner
            .seek(io::SeekFrom::Start(self.map_offset + chunk_nr * 8))?;
        write_u64(&mut self.inner, crc64)?;
        self.crc_map[usize::cast_from(chunk_nr)?] = crc64;
        Ok(())
    }

    pub fn check_chunk(&mut self, chunk_nr: u32) -> Result<bool, Error> {
        use crc::Hasher64;

        if chunk_nr >= self.meta.chunks {
            Err(Error::Overflow)
        } else {
            let meta_crc64 = self.crc_map[chunk_nr as usize];

            if meta_crc64 == 0 {
                return Ok(false);
            }

            let (mut from, to) = self.chunk(chunk_nr);

            let mut digest = crc::crc64::Digest::new(crc::crc64::ISO);
            let mut buf = [0u8; 4096];

            self.inner.seek(io::SeekFrom::Start(from))?;

            while from < to {
                let n_bytes = if from + 4096 > to {
                    let chunk_size = (to - from) as usize;
                    self.inner.read(&mut buf[0..chunk_size])?
                } else {
                    self.inner.read(&mut buf[..])?
                };

                digest.write(&buf[0..n_bytes]);
                from += n_bytes as u64;
            }

            let chunk_crc64 = digest.sum64();

            let valid = if chunk_crc64 == 0 {
                meta_crc64 == ::std::u64::MAX
            } else {
                meta_crc64 == chunk_crc64
            };

            if !valid {
                self.crc_map[chunk_nr as usize] = 0;
            }
            Ok(valid)
        }
    }

    pub fn finish(self) -> Result<(), Error> {
        self.inner.set_len(self.meta.size)?;
        let file_name = self.meta.file_name;
        drop(self.inner);
        fs::rename(self.temp_file_name, &file_name)?;
        Ok(())
    }
}

struct ProxyInner<T> {
    cpu_pool: CpuPool,
    after: RefCell<oneshot::Receiver<Box<Option<T>>>>,
}

pub(super) struct Proxy<T> {
    inner: Arc<ProxyInner<T>>,
}

impl<T> Clone for Proxy<T> {
    fn clone(&self) -> Self {
        Proxy {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Proxy<T>
where
    T: Send + 'static,
{
    pub fn new<
        E: From<oneshot::Canceled> + Send + 'static,
        Builder: FnOnce() -> Result<T, E> + Send + 'static,
    >(
        cpu_pool: CpuPool,
        builder: Builder,
    ) -> impl Future<Item = Self, Error = E> {
        let (tx, after) = oneshot::channel();

        cpu_pool
            .spawn_fn(move || {
                let instance = Box::new(Some(builder()?));
                tx.send(instance);
                Ok(())
            })
            .and_then(move |()| {
                Ok(Proxy {
                    inner: Arc::new(ProxyInner {
                        cpu_pool,
                        after: RefCell::new(after),
                    }),
                })
            })
    }

    pub fn with<F, R>(&self, f: F) -> impl Future<Item = R, Error = oneshot::Canceled> + 'static
    where
        F: FnOnce(&mut T) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();

        let cpu_pool = self.inner.cpu_pool.clone();
        let after = self.inner.after.replace(rx);
        //let after = ::std::mem::replace(&mut self.after, rx);
        //
        let new_fut = after.and_then(move |mut it| {
            cpu_pool.spawn_fn(move || {
                let r = f(it.as_mut().as_mut().unwrap());
                tx.send(it);
                Ok(r)
            })
        });

        new_fut
    }

    pub fn close<F, R>(self, f: F) -> impl Future<Item = R, Error = oneshot::Canceled> + 'static
    where
        F: FnOnce(T) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();

        let cpu_pool = self.inner.cpu_pool.clone();
        let after = self.inner.after.replace(rx);
        //let after = ::std::mem::replace(&mut self.after, rx);
        //
        let new_fut = after.and_then(move |mut it| {
            cpu_pool.spawn_fn(move || {
                let r = f(it.take().unwrap());
                tx.send(it);
                Ok(r)
            })
        });

        new_fut
    }
}
