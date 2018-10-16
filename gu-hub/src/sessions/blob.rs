use super::responses::*;
use actix_web::dev::Payload;
use actix_web::fs::NamedFile;
use futures::future;
use futures::Future;
use gu_base::files::write_async;
use std::fs::File;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Blob {
    path: PathBuf,
    sent: bool,
}

impl Blob {
    pub fn new(path: PathBuf) -> Blob {
        // TODO:
        let _ = File::create(&path);

        Blob { path, sent: false }
    }

    pub fn write(mut self, fut: Payload) -> impl Future<Item = Blob, Error = SessionErr> {
        future::ok(self.path.clone()).and_then(|path| {
            let path2 = path.clone();
            write_async(fut, path2)
                .map_err(move |e| SessionErr::FileError(e))
                .and_then(move |_| {
                    self.sent = true;
                    Ok(self)
                })
        })
    }

    pub fn read(self) -> Result<NamedFile, SessionErr> {
        if !self.sent {
            return Err(SessionErr::BlobLockedError);
        }

        NamedFile::open(&self.path).map_err(|e| SessionErr::FileError(e.to_string()))
    }
}
