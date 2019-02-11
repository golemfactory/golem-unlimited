/*
    Smart downloader
*/
use actix::prelude::*;
use bytes::*;
use failure::Fail;
use futures::prelude::*;
use futures_cpupool::CpuPool;
use gu_actix::prelude::*;
use gu_actix::safe::*;
use serde_derive::*;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::mem::size_of;
use std::{fmt, fs, io, path, time};

mod error;
mod sync_io;

use self::sync_io::{CheckType, DownloadFile, LogMetadata, Proxy};

pub use self::error::Error;
use actix_web::{http::header, HttpMessage};
use derive_builder::*;
use std::sync::Arc;

#[derive(Builder, Clone)]
pub struct DownloadOptions {
    #[builder(default = "5")]
    connect_retry: u16,
    #[builder(default = "524288")]
    chunk_size: u32,
    #[builder(default = "time::Duration::from_secs(120)")]
    chunk_timeout: time::Duration,
    #[builder(default = "3")]
    connections: u16,
}

impl DownloadOptions {
    pub fn download(
        self,
        url: &str,
        dest_file: String,
    ) -> impl Stream<Item = ProgressStatus, Error = Error> {
        download(self, url, dest_file)
    }
}

impl DownloadOptionsBuilder {
    pub fn download(
        &self,
        url: &str,
        dest_file: String,
    ) -> impl Stream<Item = ProgressStatus, Error = Error> {
        let url = url.to_owned();

        self.build()
            .into_future()
            .and_then(move |o| Ok(o.download(&url, dest_file)))
            .map_err(|e| Error::Other(e.into()))
            .flatten_stream()
    }
}

pub fn cpu_pool() -> CpuPool {
    actix_web::server::ServerSettings::default()
        .cpu_pool()
        .clone()
}

trait ProgressReport: Clone {
    fn progress(progress: ProgressStatus);
}

fn download_chunk(
    meta: Arc<LogMetadata>,
    options: Arc<DownloadOptions>,
    proxy: Proxy<DownloadFile>,
    chunk_nr: u32,
    from: u64,
    to: u64,
) -> impl Future<Item = Chunk, Error = Error> {
    use actix_web::{client, http::header, HttpMessage};
    use futures::future::{self, loop_fn, Loop};
    let limit = (to - from) as usize;

    loop_fn(options.connect_retry, move |n_retries| {
        let proxy = proxy.clone();
        let meta = meta.clone();
        let options = options.clone();
        let size = meta.size;

        proxy
            .with(move |df| df.check_chunk(chunk_nr))
            .from_err()
            .and_then(move |v| match v {
                Ok(true) => {
                    return future::Either::A(future::ok(Loop::Break(Chunk { chunk_nr, from, to })));
                }
                _ => future::Either::B(
                    client::get(&meta.url)
                        .header(header::IF_RANGE, format!("{}", meta.to_if_range().unwrap()))
                        .header(header::RANGE, format!("bytes={}-{}", from, to - 1))
                        .finish()
                        .unwrap()
                        .send()
                        .timeout(options.chunk_timeout)
                        .map_err(|e| Error::Other(format!("{}", e)))
                        .and_then(move |resp| {
                            resp.body()
                                .limit(limit)
                                .map_err(|e| Error::Other(format!("resp: {}", e)))
                        })
                        .and_then(move |bytes| {
                            proxy
                                .with(move |df| df.add_chunk(from, to, bytes.as_ref()))
                                .from_err()
                        })
                        .and_then(move |v| Ok(Loop::Break(Chunk { chunk_nr, from, to })))
                        .or_else(move |e| {
                            if n_retries > 0 {
                                Ok(Loop::Continue(n_retries - 1))
                            } else {
                                Err(e)
                            }
                        }),
                ),
            })
    })
}

struct UrlInfo {
    download_url: String,
    size: Option<u64>,
    check: CheckType,
    accept_ranges: bool,
}

fn extract_check<M: actix_web::HttpMessage>(resp: &M) -> Result<CheckType, header::ToStrError> {
    if let Some(etag_value) = resp.headers().get(actix_web::http::header::ETAG) {
        Ok(CheckType::ETag(etag_value.to_str()?.into()))
    } else if let Some(last_mod_time) = resp.headers().get(header::LAST_MODIFIED) {
        Ok(CheckType::ModTime(last_mod_time.to_str()?.into()))
    } else {
        Ok(CheckType::None)
    }
}

/*
  Prepare to download

*/
pub fn check_url(url: &str) -> impl Future<Item = UrlInfo, Error = Error> {
    use actix_web::{client, http::header, HttpMessage};
    use futures::future::{loop_fn, Loop};

    loop_fn((url.to_owned(), 0), |(url, retry)| {
        client::get(&url)
            .header("user-agent", "gu-downloader")
            .finish()
            .into_future()
            .from_err()
            .and_then(|request| request.send().from_err())
            .and_then(move |resp| {
                if resp.status().is_redirection() {
                    if let Some(Ok(location)) = resp
                        .headers()
                        .get(header::LOCATION)
                        .map(header::HeaderValue::to_str)
                    {
                        // TODO: Relative URL
                        if retry < 3 {
                            Ok(Loop::Continue((location.into(), retry + 1)))
                        } else {
                            Err(Error::Other("too many retries".into()))
                        }
                    } else {
                        Err(Error::Other("too many retries".into()))
                    }
                } else if resp.status().is_success() {
                    let headers = resp.headers();
                    let check = extract_check(&resp)
                        .map_err(|e| Error::Other(format!("invalid response: {}", e)))?;
                    let size: Option<u64> = headers
                        .get("content-length")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse().ok());
                    Ok(Loop::Break(UrlInfo {
                        download_url: url,
                        size,
                        check,
                        accept_ranges: true,
                    }))
                } else {
                    Err(Error::Other(format!(
                        "invalid response status: {}",
                        resp.status()
                    )))
                }
            })
    })
}

fn download(
    options: DownloadOptions,
    url: &str,
    dest_file: String,
) -> impl Stream<Item = ProgressStatus, Error = Error> {
    use actix_web::http::header::HeaderValue;
    use actix_web::http::HttpTryFrom;
    use actix_web::HttpMessage;

    use futures::{prelude::*, stream, unsync::mpsc};

    let (tx, rx) = mpsc::unbounded();

    let init_tx = tx.clone();
    let options = Arc::new(options);
    let connections = options.connections;

    let chunks_stream = check_url(url)
        .and_then(|info| {
            Proxy::new(cpu_pool(), move || {
                DownloadFile::new(
                    dest_file.as_ref(),
                    &info.download_url,
                    info.check,
                    info.size.unwrap(),
                )
            })
            .from_err()
        })
        .and_then(move |mut download_file| {
            download_file
                .with(|df: &mut DownloadFile| {
                    let meta = df.meta();
                    let v: Vec<(u64, u64, u32)> = (0..meta.chunks)
                        .map(|n| {
                            let (from, to) = df.chunk(n);
                            (from, to, n)
                        })
                        .collect();

                    (Arc::new(meta), v)
                })
                .and_then(move |(meta, chunks)| {
                    init_tx.unbounded_send(ProgressStatus {
                        total_to_download: Some(meta.size),
                        downloaded_bytes: 0,
                    });
                    Ok((download_file, meta, chunks))
                })
                .from_err()
        })
        .and_then(
            move |(download_file, meta, chunks): (Proxy<DownloadFile>, _, _)| {
                let init_progress = ProgressStatus {
                    total_to_download: Some(meta.size),
                    downloaded_bytes: 0,
                };
                let df = download_file.clone();

                eprintln!("do");
                stream::iter_ok(chunks.into_iter().map(move |(from, to, n)| {
                    download_chunk(
                        meta.clone(),
                        options.clone(),
                        download_file.clone(),
                        n,
                        from,
                        to,
                    )
                }))
                .buffer_unordered(connections as usize)
                .fold(init_progress, move |mut progress, chunk| {
                    eprintln!("progress: {:?} / {:?}", progress, chunk);
                    progress.downloaded_bytes += (chunk.to - chunk.from);
                    tx.unbounded_send(progress.clone());
                    Ok::<_, Error>(progress)
                })
                .and_then(|_| df.close(|df| df.finish()).flatten_fut())
            },
        );

    Arbiter::spawn(
        chunks_stream
            .and_then(|_| Ok(()))
            .map_err(|e| eprintln!("ERR: {}", e)),
    );

    rx.map_err(|e| Error::Other(format!("e")))
}

#[derive(Clone, Debug)]
struct Chunk {
    chunk_nr: u32,
    from: u64,
    to: u64,
}

#[derive(Clone, Debug)]
pub struct ProgressStatus {
    pub downloaded_bytes: u64,
    pub total_to_download: Option<u64>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_builder() {
        let b: DownloadOptions = DownloadOptionsBuilder::default()
            .chunk_size(3000)
            .build()
            .unwrap();

        assert_eq!(b.connect_retry, 5);
        assert_eq!(b.chunk_timeout, time::Duration::from_secs(120));
        assert_eq!(b.chunk_size, 3000);
    }

}
