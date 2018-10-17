use actix::{Actor, Context, Handler, Message, Supervised, SystemService};
use bytes::Bytes;
use futures::future;
use futures::prelude::*;
use futures::Async;
use futures_cpupool::CpuPool;
use sha1::Sha1;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;

struct FileWriter {
    pool: CpuPool,
}

impl Default for FileWriter {
    fn default() -> FileWriter {
        FileWriter {
            pool: CpuPool::new_num_cpus(),
        }
    }
}

impl Supervised for FileWriter {}

impl SystemService for FileWriter {}

impl Actor for FileWriter {
    type Context = Context<Self>;
}

struct WriteToFile {
    file: File,
    x: Bytes,
    pos: u64,
}

impl Message for WriteToFile {
    type Result = ();
}

impl Handler<WriteToFile> for FileWriter {
    type Result = ();

    fn handle(&mut self, msg: WriteToFile, ctx: &mut Context<Self>) -> () {
        use actix::AsyncContext;
        use actix::WrapFuture;

        ctx.spawn(
            write_chunk_on_pool(msg.file, msg.x, msg.pos, self.pool.clone())
                .map_err(|_| ())
                .into_actor(self),
        );
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
    stream_with_positions(input_stream, path).fold(Sha1::new(), move |mut sha, (x, pos, file)| {
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
    FileWriter::from_registry()
        .send(msg)
        .map_err(|e| format!("FileWriter error: {:?}", e))
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
