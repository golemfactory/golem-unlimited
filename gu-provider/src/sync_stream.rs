//! Implementation for sync stream feed

use bytes::{BufMut, Bytes, BytesMut};
use crossbeam_channel as cb;
use futures::prelude::*;
use futures::{Async, Poll};
use futures_cpupool::{CpuFuture, CpuPool};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::mem;
use std::sync::{Arc, Mutex};

pub trait SyncWriteWorker {
    fn bind_writer(&mut self, w: SyncBuffer);

    fn write_chunk(&mut self) -> io::Result<bool>;
}

pub struct SyncBuffer {
    bytes: BytesMut,
    output: Arc<Mutex<VecDeque<Bytes>>>,
}

impl Write for SyncBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let size = buf.len();

        self.bytes.reserve(size);
        self.bytes.put_slice(buf);
        if self.bytes.len() > 700 {
            self.flush()?
        }
        Ok(size)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.bytes.len() > 0 {
            let bytes = mem::replace(&mut self.bytes, BytesMut::with_capacity(1024));
            self.output.lock().unwrap().push_back(bytes.freeze());
        }
        Ok(())
    }
}

struct SyncWorkerAdapter<W: SyncWriteWorker + Send> {
    output: Arc<Mutex<VecDeque<Bytes>>>,
    w: Option<W>,
}

impl<W: SyncWriteWorker + Send> SyncWorkerAdapter<W> {
    fn new(mut w: W) -> Self {
        let output = Arc::new(Mutex::new(VecDeque::new()));
        let buffer = SyncBuffer {
            bytes: BytesMut::new(),
            output: output.clone(),
        };
        w.bind_writer(buffer);
        SyncWorkerAdapter { output, w: Some(w) }
    }

    fn is_empty(&self) -> bool {
        self.output.lock().unwrap().is_empty()
    }
}

impl<W: SyncWriteWorker + Send> Future for SyncWorkerAdapter<W> {
    type Item = Option<SyncWorkerAdapter<W>>;
    type Error = io::Error;

    fn poll(&mut self) -> Result<Async<<Self as Future>::Item>, <Self as Future>::Error> {
        while self.is_empty() {
            if !self.w.as_mut().unwrap().write_chunk()? {
                return Ok(Async::Ready(None));
            }
        }
        let w = self.w.take();
        Ok(Async::Ready(Some(SyncWorkerAdapter {
            output: self.output.clone(),
            w,
        })))
    }
}

struct SyncStream<W: SyncWriteWorker + Send> {
    worker: Option<SyncWorkerAdapter<W>>,
    pending_work: Option<CpuFuture<Option<SyncWorkerAdapter<W>>, io::Error>>,
    output: Arc<Mutex<VecDeque<Bytes>>>,
    cpu_pool: CpuPool,
}

impl<W: SyncWriteWorker + Send + 'static> SyncStream<W> {
    fn new(w: W) -> Self {
        let settings = actix_web::server::ServerSettings::default();
        let worker = SyncWorkerAdapter::new(w);
        let output = worker.output.clone();

        SyncStream {
            worker: Some(worker),
            pending_work: None,
            output,
            cpu_pool: settings.cpu_pool().clone(),
        }
    }

    fn get_output(&mut self) -> Option<Bytes> {
        self.output.lock().unwrap().pop_front()
    }

    fn poll_chunk_sync(&mut self) -> Poll<Option<Bytes>, io::Error> {
        if let Some(worker) = self.worker.take() {
            self.pending_work = Some(self.cpu_pool.spawn(worker))
        }
        if let Some(mut future) = self.pending_work.take() {
            match future.poll() {
                Ok(Async::Ready(None)) => Ok(Async::Ready(self.get_output())),
                Ok(Async::Ready(Some(w))) => {
                    self.worker = Some(w);
                    self.poll_chunk()
                }
                Ok(Async::NotReady) => {
                    self.pending_work = Some(future);
                    Ok(Async::NotReady)
                }
                Err(e) => Err(e),
            }
        } else {
            panic!("no work")
        }
    }

    fn poll_chunk(&mut self) -> Poll<Option<Bytes>, io::Error> {
        let output = self.get_output();
        if output.is_some() {
            Ok(Async::Ready(output))
        } else {
            self.poll_chunk_sync()
        }
    }
}

impl<W: SyncWriteWorker + Send + 'static> Stream for SyncStream<W> {
    type Item = Bytes;
    type Error = io::Error;

    fn poll(&mut self) -> Result<Async<Option<<Self as Stream>::Item>>, <Self as Stream>::Error> {
        self.poll_chunk()
    }
}

pub fn sync_stream<W: SyncWriteWorker + Send + 'static>(
    w: W,
) -> impl Stream<Item = Bytes, Error = io::Error> {
    SyncStream::new(w)
}

struct InlineWorker<I, S> {
    i: Option<I>,
    s: Option<S>,
}

impl<
        I: FnOnce(SyncBuffer) -> S + Send + 'static,
        S: FnMut() -> io::Result<bool> + Send + 'static,
    > SyncWriteWorker for InlineWorker<I, S>
{
    fn bind_writer(&mut self, w: SyncBuffer) {
        self.s = Some((self.i.take().unwrap())(w));
    }

    fn write_chunk(&mut self) -> Result<bool, io::Error> {
        (self.s.as_mut().unwrap())()
    }
}

pub fn stream_fn<
    I: FnOnce(SyncBuffer) -> S + Send + 'static,
    S: FnMut() -> io::Result<bool> + Send + 'static,
>(
    i: I,
) -> impl Stream<Item = Bytes, Error = io::Error> {
    sync_stream(InlineWorker {
        i: Some(i),
        s: None,
    })
}
