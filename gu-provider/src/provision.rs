use actix_web::client::ClientResponse;
use actix_web::http::header;
use actix_web::HttpMessage;
use futures::{future, prelude::*};
use gu_actix::{async_result, async_try};
use gu_base::files::read_async;
use gu_base::files::{untgz_async, write_async};
use gu_model::envman::ResourceFormat;
use log::{debug, info};
use std::{
    fs,
    path::{Path, PathBuf},
    time,
};

pub fn download_step(
    url: &str,
    output_path: PathBuf,
    format: ResourceFormat,
) -> impl Future<Item = (), Error = String> {
    use actix_web::client;
    use tar_async::decode::full;

    let client_request = async_try!(client::ClientRequest::get(url)
        .finish()
        .map_err(|e| format!("{}", e)));

    let dir_name = match format {
        ResourceFormat::Raw => output_path.parent().unwrap(),
        ResourceFormat::Tar => output_path.as_ref(),
    };

    if !dir_name.exists() {
        async_try!(fs::create_dir_all(dir_name).map_err(|e| format!("create dir {}", e)))
    }

    future::Either::A(
        client_request
            .send()
            .map_err(|e| format!("send download request: {}", e))
            .and_then(move |resp| {
                match format {
                    ResourceFormat::Raw => future::Either::A(
                        write_async(resp.payload(), output_path)
                            .map_err(|_| "writing downloaded file failed".to_string()),
                    ),
                    ResourceFormat::Tar => future::Either::B(
                        full::decode_tar(resp.payload())
                            .map_err(|e| format!("tar: {}", e))
                            .for_each(move |entry| {
                                let entry_type = entry.header().entry_type().clone();
                                let path: PathBuf = async_try!(entry
                                    .header()
                                    .path()
                                    .map_err(|e| format!("payload err: {}", e)))
                                .to_owned();
                                eprintln!("tar-path:{}", path.display());

                                if entry_type.is_dir() {
                                    // is directory
                                    use std::fs;
                                    let dir_name = output_path.join(path);
                                    if !dir_name.exists() {
                                        let _ = async_try!(fs::create_dir_all(dir_name)
                                            .map_err(|e| format!("io: {}", e)));
                                    }
                                    future::Either::B(future::ok(()))
                                } else if entry_type.is_file() {
                                    let out_file = output_path.join(path);
                                    async_result!(write_async(entry, out_file))
                                } else {
                                    // if entry.header().path() { }
                                    future::Either::B(future::ok(()))
                                }
                            }),
                    ),
                }
            }),
    )
}

pub fn upload_step(
    url: &str,
    input_path: PathBuf,
    format: ResourceFormat,
) -> impl Future<Item = String, Error = String> {
    use actix_web::{client, error::ErrorInternalServerError};

    debug!(
        "streaming from {:?} to {} format: {:?}",
        &input_path, url, format
    );
    let source_stream: Box<dyn Stream<Item = bytes::Bytes, Error = String>> = match format {
        ResourceFormat::Tar => Box::new(stream_tar(input_path)),
        ResourceFormat::Raw => Box::new(stream_raw(input_path)),
    };
    let url_desc = url.to_owned();

    future::result(
        client::put(url).streaming(source_stream.map_err(|x| ErrorInternalServerError(x))),
    )
    .map_err(|e| e.to_string())
    .and_then(|req| req.send().map_err(|e| e.to_string()))
    .and_then(move |res| {
        if res.status().is_success() {
            Ok(format!("{:?} file uploaded", url_desc))
        } else {
            Err(format!("Unsuccessful file upload: {}", res.status()))
        }
    })
}

pub fn stream_tar(input_path: PathBuf) -> impl Stream<Item = bytes::Bytes, Error = String> {
    use gu_actix::pipe;
    use std::thread;
    use tar::Builder;

    let (tx, rx) = pipe::sync_to_async(5);

    thread::spawn(move || {
        let mut builder = Builder::new(tx);
        builder
            .append_dir_all(".", &input_path)
            .unwrap_or_else(|e| {
                panic!("Error while building the archive: {}", e);
            });
        builder.finish().unwrap();
    });

    rx.map_err(|e| {
        eprintln!("error={}", e);
        e.to_string()
    })
}

fn stream_raw(input_path: PathBuf) -> impl Stream<Item = bytes::Bytes, Error = String> {
    read_async(input_path)
}

pub fn untar_single_file_stream<TarStream: Stream<Item = bytes::Bytes>>(
    stream: TarStream,
) -> impl Future<Item = (u64, impl Stream<Item = bytes::Bytes, Error = String>), Error = String>
where
    TarStream::Error: std::fmt::Debug + Sync + Send + 'static,
{
    use tar_async::decode::flat::{self, TarItem};

    flat::decode_tar(stream)
        .into_future()
        .map_err(|(e, tail)| e.to_string())
        .and_then(|(head, tail)| match head {
            Some(TarItem::Entry(entry)) => Ok((
                entry.size(),
                tail.map_err(|e| e.to_string()).and_then(|item| match item {
                    TarItem::Entry(_) => Err("tar contains more than one file".to_string()),
                    TarItem::Chunk(bytes) => Ok(bytes),
                }),
            )),
            _ => Err("invalid tar response".to_string()),
        })
}

// TODO: support redirect
// TODO: support https
pub fn download(
    url: &str,
    output_path: PathBuf,
    use_cache: bool,
) -> impl Future<Item = (), Error = String> {
    info!("downloading from {} to {:?}", url, &output_path);
    use actix_web::client;

    if use_cache && output_path.exists() {
        info!("using cached file {:?}", &output_path);
        return future::Either::A(future::ok(()));
    }

    let client_request = client::ClientRequest::get(url).finish().unwrap();

    future::Either::B(
        client_request
            .send()
            .conn_timeout(time::Duration::from_secs(15))
            .timeout(time::Duration::from_secs(3600))
            .map_err(|e| format!("send download request: {}", e))
            .and_then(|resp| {
                write_async(resp.payload(), output_path)
                    .map_err(|_| "writing downloaded file failed".to_string())
            }),
    )
}

fn content_length(r: &ClientResponse) -> Result<u64, String> {
    r.headers()
        .get(header::CONTENT_LENGTH)
        .ok_or("Downloaded file does not have content-length header")
        .and_then(|header| {
            header
                .to_str()
                .map_err(|_| "Incorrect ascii text in content-length header")
        })
        .and_then(|text: &str| {
            text.parse::<u64>()
                .map_err(|_| "Incorrect number in content-length header")
        })
        .map_err(|e| e.to_string())
}

fn response_to_tarred_stream<P>(
    resp: ClientResponse,
    path: P,
) -> impl Stream<Item = bytes::Bytes, Error = String> + 'static
where
    P: AsRef<Path>,
{
    let header = content_length(&resp).and_then(|length| {
        let mut header = tar::Header::new_ustar();
        header.set_size(length);
        header
            .set_path(path)
            .map_err(|_| "Incorrect filepath - cannot be set as filepath in tar".to_string())?;
        header.set_cksum();

        let header: &[u8] = header.as_bytes();
        Ok(bytes::Bytes::from(header))
    });

    futures::stream::once(header).chain(resp.payload().map_err(|e| e.to_string()))
}

fn response_to_stream(
    resp: ClientResponse,
) -> impl Stream<Item = bytes::Bytes, Error = String> + 'static {
    resp.payload().map_err(|e| e.to_string())
}

fn inner_download_stream<F, S>(
    url: &str,
    function: F,
) -> impl Stream<Item = bytes::Bytes, Error = String> + 'static
where
    F: Fn(ClientResponse) -> S + 'static,
    S: Stream<Item = bytes::Bytes, Error = String> + 'static,
{
    use actix_web::client;

    let client_request = client::ClientRequest::get(url).finish().unwrap();

    client_request
        .send()
        .timeout(time::Duration::from_secs(300))
        .map_err(|e| e.to_string())
        .map(move |resp| function(resp))
        .flatten_stream()
}

pub fn download_stream(url: &str) -> impl Stream<Item = bytes::Bytes, Error = String> + 'static {
    inner_download_stream(url, response_to_stream)
}

pub fn tarred_download_stream<P>(
    url: &str,
    filename: P,
) -> impl Stream<Item = bytes::Bytes, Error = String> + 'static
where
    P: AsRef<Path> + Clone + 'static,
{
    inner_download_stream(url, move |resp| {
        response_to_tarred_stream(resp, filename.clone())
    })
}

pub fn untgz<P: AsRef<Path> + ToOwned>(
    input_path: P,
    output_path: P,
) -> impl Future<Item = (), Error = String> {
    info!(
        "untgz from {:?} to {:?}",
        input_path.as_ref(),
        output_path.as_ref()
    );

    untgz_async(input_path, output_path)
}
