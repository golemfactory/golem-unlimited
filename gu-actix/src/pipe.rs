use crossbeam_channel::{self as cb, Receiver, Sender};
use std::sync::Arc;
use futures::task::AtomicTask;
use futures::{Poll, Stream, Async};
use std::io;

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

impl io::Write for Writer<bytes::Bytes, io::Error> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        use bytes::Bytes;
        self.send(Ok(Bytes::from(buf))).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}


pub fn channel<T, E>(cap: usize) -> (Writer<T, E>, AsyncReader<T, E>) {
    let (tx, rx) = cb::bounded(cap);
    let task = Arc::new(AtomicTask::new());

    (Writer {
        tx,
        task: task.clone(),
    }, AsyncReader {
        rx,
        task: task.clone(),
    })
}
