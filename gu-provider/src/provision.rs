use actix_web::HttpMessage;
use futures::{future, prelude::*};
use gu_base::files::read_async;
use gu_base::files::{untgz_async, write_async};
use gu_model::envman::ResourceFormat;
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

    if !output_path.exists() {
        async_try!(fs::create_dir_all(&output_path).map_err(|e| format!("creare dir {}", e)))
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
                            })
                            .map(|_| ()),
                    ),
                }
            }),
    )
}

pub fn upload_step(
    uri: &str,
    input_path: PathBuf,
    format: ResourceFormat,
) -> impl Future<Item = (), Error = String> {
    use actix_web::{client, error::ErrorInternalServerError};

    let source_stream: Box<dyn Stream<Item = bytes::Bytes, Error = String>> = match format {
        ResourceFormat::Tar => Box::new(stream_tar(input_path)),
        ResourceFormat::Raw => Box::new(stream_raw(input_path)),
    };

    let client_request = async_try!(client::put(uri)
        .streaming(source_stream.map_err(|e| ErrorInternalServerError(e)))
        .map_err(|e| format!("{}", e)));

    async_result!(future::ok(()))
}

pub fn stream_tar(input_path: PathBuf) -> impl Stream<Item = bytes::Bytes, Error = String> {
    use gu_actix::pipe;
    use std::fs;
    use std::thread;
    use tar::Builder;

    let (tx, rx) = pipe::sync_to_async(5);

    thread::spawn(move || {
        let mut builder = Builder::new(tx);
        builder.append_dir_all(".", &input_path).unwrap();
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
