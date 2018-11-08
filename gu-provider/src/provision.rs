use actix_web::HttpMessage;
use futures::{future, prelude::*};
use gu_base::files::{untgz_async, write_async};
use std::{
    path::{Path, PathBuf},
    time,
};

// TODO: support redirect
// TODO: support https
pub fn download(url: &str, output_path: PathBuf) -> impl Future<Item = (), Error = String> {
    info!("downloading from {} to {:?}", url, &output_path);
    use actix_web::client;

    if output_path.exists() {
        info!("using cached file {:?}", &output_path);
        return future::Either::A(future::ok(()));
    }

    let client_request = client::ClientRequest::get(url).finish().unwrap();

    future::Either::B(
        client_request
            .send()
            .timeout(time::Duration::from_secs(300))
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
