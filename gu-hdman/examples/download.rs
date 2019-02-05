use actix::prelude::*;
use futures::stream::Stream;
use gu_hdman::download::DownloadOptionsBuilder;
use pbr::ProgressBar;
use std::cell::RefCell;
use std::rc::Rc;
use structopt::*;

#[derive(Debug, StructOpt)]
#[structopt(name = "gu-download", about = "An example of download operation")]
struct DownloadOpts {
    download_url: String,
}

fn main() {
    let opt = DownloadOpts::from_args();
    let mut sys = System::new("test");

    let mut progress = Rc::new(RefCell::new(None));

    let progress_ref = progress.clone();

    sys.block_on(
        DownloadOptionsBuilder::default()
            .download(
                &opt.download_url,
                //"http://52.31.143.91/images/x86_64/linux/gu-blend.hdi",
                "/tmp/gu-blend.hdi".into(),
            )
            .map_err(|e| eprintln!("err: {}", e))
            .for_each(move |p| {
                let mut p_ref = progress.borrow_mut();

                let mut pp = match p_ref.take() {
                    None => ProgressBar::new(p.total_to_download.unwrap()),
                    Some(p) => p,
                };
                pp.set_units(pbr::Units::Bytes);
                pp.set(p.downloaded_bytes);

                p_ref.replace(pp);

                /*
                            println!(
                                "progress: {}/{}",
                                p.downloaded_bytes,
                                p.total_to_download.unwrap()
                            );
                */
                Ok(())
            }),
    );

    if let Some(mut pg) = progress_ref.replace(None) {
        pg.finish_print("done");
    }
}
