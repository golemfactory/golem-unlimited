//! Smart upload for hdman provision
//!

use bytes::*;
use futures::prelude::*;
use std::io::Write;
use std::{fs, io, time};

pub struct Downloader {}

pub struct ProgressStatus {
    pub downloaded_bytes: u64,
    pub total_to_download: Option<u64>,
    pub download_duration: time::Duration,
}

impl Downloader {
    pub fn progress(&self) -> impl Stream<Item = ProgressStatus, Error = ()> {
        futures::stream::empty()
    }
}

impl Future for Downloader {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        unimplemented!()
    }
}

struct LogFile {
    inner: fs::File,
}

#[derive(Serialize, Deserialize)]
enum LogEntry<'a> {
    Begin {
        url: &'a str,
        file_name: &'a str,
    },
    Download {
        etag: Option<&'a str>,
        size: Option<u64>,
    },
    Chunk {
        from: u64,
        to: u64,
    },
    Eof {
        to: u64,
    },
}

impl LogFile {
    const RECORD_BEGIN: &'static [u8; 4] = b"guLF";

    fn write_record<'a>(&mut self, entry: &LogEntry<'a>) -> io::Result<()> {
        let v = serde_json::to_vec(entry).unwrap();
        let len = v.len() as u32;
        self.inner.write_all(Self::RECORD_BEGIN)?;
        self.inner.write_all(&len.to_be_bytes()[..]);

        Ok(())
    }
}
