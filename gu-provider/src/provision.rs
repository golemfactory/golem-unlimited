use actix_web::HttpMessage;
use futures::future;
use futures::prelude::*;
use std::path::{Path, PathBuf};
use std::time;

// TODO: support redirect
// TODO: support https
pub fn download(url: &str, output_path: PathBuf) -> Box<Future<Item = (), Error = String>> {
    info!("downloading from {} to {:?}", url, &output_path);
    use actix_web::client;
    use write_to::to_file;

    if output_path.exists() {
        info!("using cached file {:?}", &output_path);
        return Box::new(future::ok(()));
    }

    let client_request = client::ClientRequest::get(url).finish().unwrap();

    Box::new(
        client_request
            .send()
            .timeout(time::Duration::from_secs(300))
            .map_err(|e| format!("send download request: {}", e))
            .and_then(|resp| {
                to_file(resp.payload(), output_path)
                    .map_err(|e| format!("download write to file: {}", e))
            }),
    )
}

pub fn untgz<P: AsRef<Path>>(input_path: P, output_path: P) -> Result<(), String> {
    use flate2::read::GzDecoder;
    use std::fs;
    use tar::Archive;

    info!(
        "untgz from {:?} to {:?}",
        input_path.as_ref(),
        output_path.as_ref()
    );
    let d = GzDecoder::new(fs::File::open(input_path).map_err(|e| e.to_string())?);
    let mut ar = Archive::new(d);
    ar.unpack(output_path).map_err(|e| e.to_string())
}
