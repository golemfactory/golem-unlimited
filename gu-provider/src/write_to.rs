use bytes::Bytes;
use futures::prelude::*;
use futures::Async;
use std::fs::File;
use std::io::Write;
use std::path::Path;

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
