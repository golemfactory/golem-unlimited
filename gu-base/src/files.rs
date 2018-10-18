use bytes::Bytes;
use futures::future;
use futures::prelude::*;
use futures::Async;
use futures_cpupool::CpuPool;
use sha1::Sha1;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;
use std::cmp;
use futures_cpupool::CpuFuture;

static mut FILE_HANDLER: Option<FilePoolHandler> = None;

fn handler() -> FilePoolHandler {
    unsafe {
        if FILE_HANDLER.is_none() {
            FILE_HANDLER = Some(FilePoolHandler::default());
        }

        FILE_HANDLER.clone().unwrap()
    }
}

#[derive(Clone)]
struct FilePoolHandler {
    pool: CpuPool,
}

impl Default for FilePoolHandler {
    fn default() -> FilePoolHandler {
        FilePoolHandler {
            pool: CpuPool::new_num_cpus(),
        }
    }
}

struct WriteToFile {
    file: File,
    x: Bytes,
    pos: u64,
}

impl FilePoolHandler {
    pub fn write_to_file(&self, msg: WriteToFile) -> impl Future<Item=(), Error=String> {
        write_chunk_on_pool(msg.file, msg.x, msg.pos, self.pool.clone())
            .map_err(|e| e.to_string())
    }

    pub fn read_file(&self, msg: ReadFile) -> impl Stream<Item=Bytes, Error=String> {
        future::result(
            match msg.range {
                Some(range) => ChunkedReadFile::new_ranged(msg.file, self.pool.clone(), range),
                None => ChunkedReadFile::new(msg.file, self.pool.clone()),
            }
        ).flatten_stream()
    }
}

fn write_chunk_on_pool(
    mut file: File,
    x: Bytes,
    pos: u64,
    pool: CpuPool,
) -> impl Future<Item = (), Error = io::Error> {
    pool.spawn_fn(move || {
        future::result(file.seek(SeekFrom::Start(pos)))
            .and_then(move |_| file.write(x.as_ref()).and_then(|_| Ok(())))
    })
}

struct ReadFile {
    file: File,
    range: Option<(u64, u64)>,
}

struct WithPositions<S: Stream<Item = Bytes, Error = String>> {
    stream: S,
    pos: u64,
}

impl<S: Stream<Item = Bytes, Error = String>> WithPositions<S> {
    pub fn new(a: S) -> WithPositions<S> {
        Self { stream: a, pos: 0 }
    }
}

impl<S: Stream<Item = Bytes, Error = String>> Stream for WithPositions<S> {
    type Item = (Bytes, u64);
    type Error = String;

    fn poll(&mut self) -> Result<Async<Option<(Bytes, u64)>>, String> {
        match self.stream.poll() {
            Ok(Async::Ready(Some(x))) => {
                let len = x.len() as u64;
                let res = Ok(Async::Ready(Some((x, self.pos))));
                self.pos += len;

                res
            }
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}

fn stream_with_positions<Ins: Stream<Item = Bytes>, P: AsRef<Path>>(
    input_stream: Ins,
    path: P,
) -> impl Stream<Item = (Bytes, u64, File), Error = String> {
    future::result(File::create(path).map_err(|e| format!("File creation error: {:?}", e)))
        .and_then(|file| {
            Ok(
                WithPositions::new(input_stream.map_err(|_| format!("Input stream error")))
                    .and_then(move |(x, pos)| {
                        file.try_clone()
                            .and_then(|file| Ok((x, pos, file)))
                            .map_err(|e| format!("File clone error {:?}", e))
                    }),
            )
        }).flatten_stream()
}

pub fn write_async_with_sha1<Ins: Stream<Item = Bytes>, P: AsRef<Path>>(
    input_stream: Ins,
    path: P,
) -> impl Future<Item = String, Error = String> {
    stream_with_positions(input_stream, path)
        .fold(Sha1::new(), move |mut sha, (x, pos, file)| {
            sha.update(x.as_ref());
            write_bytes(x, pos, file).and_then(|_| Ok(sha))
        }).and_then(|sha| Ok(sha.digest().to_string()))
}

pub fn write_async<Ins: Stream<Item = Bytes>, P: AsRef<Path>>(
    input_stream: Ins,
    path: P,
) -> impl Future<Item = (), Error = String> {
    stream_with_positions(input_stream, path).for_each(|(x, pos, file)| write_bytes(x, pos, file))
}

fn write_bytes(x: Bytes, pos: u64, file: File) -> impl Future<Item = (), Error = String> {
    let msg = WriteToFile { file, x, pos };
    handler().write_to_file(msg)
        .map_err(|e| format!("FileWriter error: {}", e))
}

pub fn read_async<P: AsRef<Path>>(
    path: P,
) -> impl Stream<Item = Bytes, Error = String> {
    let file_fut = future::result(File::open(path));

    file_fut
        .map_err(|e| e.to_string())
        .and_then(|file|
        Ok(ReadFile {
            file,
            range: None,
        }))
        .and_then(|read| Ok(handler().read_file(read)))
        .flatten_stream()
}


/// https://actix.rs/api/actix-web/stable/src/actix_web/fs.rs.html#477-484
pub struct ChunkedReadFile {
    size: u64,
    offset: u64,
    cpu_pool: CpuPool,
    file: Option<File>,
    fut: Option<CpuFuture<(File, Bytes), io::Error>>,
    counter: u64,
}

impl ChunkedReadFile {
    pub fn new(file: File, pool: CpuPool) -> Result<ChunkedReadFile, String> {
        Ok(ChunkedReadFile {
            size: file.metadata().map_err(|e| e.to_string())?.len(),
            offset: 0,
            cpu_pool: pool,
            file: None,
            fut: None,
            counter: 0,
        })
    }

    pub fn new_ranged(file: File, pool: CpuPool, range: (u64, u64)) -> Result<ChunkedReadFile, String> {
        let len = file.metadata().map_err(|e| e.to_string())?.len();
        if range.0 >= range.1 || range.1 > len {
            return Err("Invalid range".to_string());
        }

        Ok(ChunkedReadFile {
            size: range.1,
            offset: range.0,
            cpu_pool: pool,
            file: None,
            fut: None,
            counter: 0,
        })
    }
}

impl Stream for ChunkedReadFile {
    type Item = Bytes;
    type Error = String;

    fn poll(&mut self) -> Poll<Option<Bytes>, String> {
        use std::io::Read;
        if self.fut.is_some() {
            return match self.fut.as_mut().unwrap().poll().map_err(|e| e.to_string())? {
                Async::Ready((file, bytes)) => {
                    self.fut.take();
                    self.file = Some(file);
                    self.offset += bytes.len() as u64;
                    self.counter += bytes.len() as u64;
                    Ok(Async::Ready(Some(bytes)))
                }
                Async::NotReady => Ok(Async::NotReady),
            };
        }

        let size = self.size;
        let offset = self.offset;
        let counter = self.counter;

        if size == counter {
            Ok(Async::Ready(None))
        } else {
            let mut file = self.file.take().expect("Use after completion");
            self.fut = Some(self.cpu_pool.spawn_fn(move || {
                let max_bytes: usize;
                max_bytes = cmp::min(size.saturating_sub(counter), 65_536) as usize;
                let mut buf = Vec::with_capacity(max_bytes);
                file.seek(io::SeekFrom::Start(offset))?;
                let nbytes =
                    io::Read::by_ref(&mut file).take(max_bytes as u64).read_to_end(&mut buf)?;
                if nbytes == 0 {
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                Ok((file, Bytes::from(buf)))
            }));
            self.poll()
        }
    }
}

#[cfg(test)]
mod tests {
    use actix::Arbiter;
    use actix::System;
    use bytes::Bytes;
    use files::write_async_with_sha1;
    use futures::prelude::*;
    use futures::stream;
    use std::path::PathBuf;

    #[test]
    #[ignore]
    fn it_works() {
        let stream = stream::iter_ok::<_, ()>(1..300).map(|a| Bytes::from(format!("{:?} ", a)));

        let _ = System::run(|| {
            Arbiter::spawn(
                write_async_with_sha1(stream, PathBuf::from("Hello World!")).then(|r| {
                    println!("{:?}", r);
                    Ok(System::current().stop())
                }),
            )
        });
    }
}
