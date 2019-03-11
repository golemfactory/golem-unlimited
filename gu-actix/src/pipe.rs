use bytes::*;
use crossbeam_channel::{self as cb, Receiver, Sender};
use futures::task::AtomicTask;
use futures::{Async, Poll, Stream};
use std::io;
use std::sync::Arc;

pub struct SyncReader<T, E> {
    rx: Receiver<Result<T, E>>,
    task: Arc<AtomicTask>,
    buffer: Option<Bytes>,
}

pub struct AsyncReader<T, E> {
    rx: Receiver<Result<T, E>>,
    task: Arc<AtomicTask>,
}

impl<T, E> Stream for AsyncReader<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<T>, E> {
        match self.rx.try_recv() {
            Ok(Ok(r)) => Ok(Async::Ready(Some(r))),
            Ok(Err(e)) => Err(e),
            Err(cb::TryRecvError::Disconnected) => Ok(Async::Ready(None)),
            Err(cb::TryRecvError::Empty) => {
                self.task.register();
                Ok(Async::NotReady)
            }
        }
    }
}

impl io::Read for SyncReader<Bytes, io::Error> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        if self.buffer.is_none() {
            let is_full = self.rx.is_full();

            let r = self.rx.recv();
            self.task.notify();

            match r {
                Ok(Ok(b)) => self.buffer = Some(b),
                Ok(Err(e)) => return Err(e),
                Err(cb::RecvError) => return Ok(0),
            }
        }
        let mut xbuf = self.buffer.take().unwrap();

        if buf.len() <= xbuf.len() {
            let chunk = xbuf.split_to(buf.len());
            buf.copy_from_slice(chunk.as_ref());
            if !xbuf.is_empty() {
                self.buffer = Some(xbuf)
            }
            Ok(buf.len())
        } else {
            let l = xbuf.len();
            buf[..l].copy_from_slice(xbuf.as_ref());
            Ok(l)
        }
    }
}

pub struct Writer<T, E> {
    tx: Sender<Result<T, E>>,
    task: Arc<AtomicTask>,
}

impl<T, E> Writer<T, E> {
    pub fn send(&mut self, r: Result<T, E>) -> Result<(), cb::SendError<Result<T, E>>> {
        self.tx.send(r)?;
        Ok(self.task.notify())
    }
}

pub struct AsyncWriter<T, E> {
    tx: Option<Sender<Result<T, E>>>,
    task: Arc<AtomicTask>,
}

pub enum WriteError<E> {
    BrokenPipe,
    Other(E),
}

impl<T, E> futures::Sink for AsyncWriter<T, E> {
    type SinkItem = T;
    type SinkError = WriteError<E>;

    fn start_send(
        &mut self,
        item: Self::SinkItem,
    ) -> Result<futures::AsyncSink<Self::SinkItem>, Self::SinkError> {
        match self.tx.as_ref().unwrap().try_send(Ok(item)) {
            Ok(()) => Ok(futures::AsyncSink::Ready),
            Err(cb::TrySendError::Full(Ok(item))) => {
                self.task.register();
                Ok(futures::AsyncSink::NotReady(item))
            }
            Err(cb::TrySendError::Disconnected(_)) => Err(WriteError::BrokenPipe),
            _ => unreachable!(),
        }
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Result<Async<()>, Self::SinkError> {
        self.tx = None;
        Ok(Async::Ready(()))
    }
}

impl io::Write for Writer<Bytes, io::Error> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        use bytes::Bytes;
        self.send(Ok(Bytes::from(buf)))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

pub fn sync_to_async<T, E>(cap: usize) -> (Writer<T, E>, AsyncReader<T, E>) {
    let (tx, rx) = cb::bounded(cap);
    let task = Arc::new(AtomicTask::new());

    (
        Writer {
            tx,
            task: task.clone(),
        },
        AsyncReader {
            rx,
            task: task.clone(),
        },
    )
}

pub fn async_to_sync<T, E>(cap: usize) -> (AsyncWriter<T, E>, SyncReader<T, E>) {
    let (tx, rx) = cb::bounded(cap);
    let task = Arc::new(AtomicTask::new());

    (
        AsyncWriter {
            tx: Some(tx),
            task: task.clone(),
        },
        SyncReader {
            rx,
            task: task.clone(),
            buffer: None,
        },
    )
}

#[cfg(test)]
mod tests {

    use super::*;
    use actix::prelude::*;
    use futures::prelude::*;
    use std::time::Duration;
    use std::{io, thread};
    use tokio_timer::Interval;

    #[test]
    fn test_channel_from() {
        let (tx, rx) = async_to_sync(1);

        thread::spawn(move || {
            use std::io::{BufRead, BufReader, Read};
            let mut buf = [0; 200];
            let mut r = BufReader::new(rx);

            eprintln!("wait");
            thread::sleep(Duration::from_secs(5));
            eprintln!("start");
            r.read_exact(&mut buf[..]).unwrap();
            eprintln!("got: {}", std::str::from_utf8(&buf[..]).unwrap())
        });

        System::run(|| {
            let f = Interval::new_interval(Duration::from_secs(1))
                .map(|x| {
                    eprintln!("it {:?}", x);
                    Bytes::from(format!("{:?}\n", x))
                })
                .map_err(|e| WriteError::Other(io::Error::new(io::ErrorKind::Other, e)))
                .forward(tx)
                .then(|_| {
                    System::current().stop();
                    Ok(eprintln!("done"))
                });
            Arbiter::spawn(f)
        });
    }

}
