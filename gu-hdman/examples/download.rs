use actix::prelude::*;
use futures::stream::Stream;
use gu_hdman::download;

fn main() -> Result<(), ()> {
    let mut sys = System::new("test");

    sys.block_on(
        download::download(
            "http://52.31.143.91/images/x86_64/linux/gu-blend.hdi",
            "/tmp/gu-blend.hdi".into(),
        )
        .map_err(|e| eprintln!("err: {}", e))
        .for_each(|p| {
            println!(
                "progress: {}/{}",
                p.downloaded_bytes,
                p.total_to_download.unwrap()
            );
            Ok(())
        }),
    )
}
