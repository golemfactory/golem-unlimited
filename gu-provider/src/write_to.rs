use bytes::Bytes;
use futures::prelude::*;
use futures::Async;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

/*
pub struct FileSink(File);

impl Sink for FileSink {
    type SinkItem = Bytes;
    type SinkError = io::Error;

    fn start_send(&mut self, item: <Self as Sink>::SinkItem) -> Result<AsyncSink<<Self as Sink>::SinkItem>, <Self as Sink>::SinkError> {
        self.0.write_all(item.as_ref())?;
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, <Self as Sink>::SinkError> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Result<Async<()>, <Self as Sink>::SinkError> {
        Ok(Async::Ready(()))
    }
}

impl FileSink {

    pub fn create<P : AsRef<Path>>(path : P) -> Result<FileSink, io::Error> {
        Ok(FileSink(File::create(path)?))
    }

}
*/

pub fn to_file<Ins: Stream<Item = Bytes>, P: AsRef<Path>>(
    input_stream: Ins,
    path: P,
) -> WriteTo<Ins, File> {
    WriteTo {
        output: File::create(path).expect("unable to create file"),
        input_stream,
    }
}

pub struct WriteTo<Ins, Outs>
where
    Ins: Stream<Item = Bytes>,
    Outs: Write,
{
    output: Outs,
    input_stream: Ins,
}

impl<Ins, Outs> Future for WriteTo<Ins, Outs>
where
    Ins: Stream<Item = Bytes>,
    Outs: ::std::io::Write,
{
    type Item = ();
    type Error = Ins::Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        let mut result = self.input_stream.poll();

        while let Ok(Async::Ready(Some(bytes))) = result {
            // TODO fix this
            self.output.write_all(&bytes.to_owned()).unwrap();
            result = self.input_stream.poll();
        }

        match result {
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
            Ok(Async::Ready(None)) => {
                debug!("done");
                Ok(Async::Ready(()))
            }
            // TODO: panic
            Ok(Async::Ready(Some(ref bytes))) => {
                self.output.write_all(bytes).unwrap();
                self.poll()
            }
        }
    }
}
