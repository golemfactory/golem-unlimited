use super::responses::*;
use actix_web::{dev::Payload, fs::NamedFile};
use futures::Future;
use gu_base::files::write_async;
use std::{fs::File, io, path::PathBuf, fs};

#[derive(Clone, Debug)]
pub struct Blob {
    path: PathBuf,
    sent: bool,
}

impl Blob {
    pub fn new(path: PathBuf) -> io::Result<Blob> {
        File::create(&path)?;

        Ok(Blob { path, sent: false })
    }

    pub fn write(mut self, fut: Payload) -> impl Future<Item = Blob, Error = SessionErr> {
        write_async(fut, self.path.clone())
            .map_err(|e| SessionErr::FileError(e))
            .and_then(move |_| {
                self.sent = true;
                Ok(self)
            })
    }

    pub fn read(self) -> Result<NamedFile, SessionErr> {
        if !self.sent {
            return Err(SessionErr::BlobLockedError);
        }

        NamedFile::open(&self.path).map_err(|e| SessionErr::FileError(e.to_string()))
    }

    pub fn clean_file(&self) -> io::Result<()> {
        match (&self.path).exists() {
            true => fs::remove_file(&self.path),
            false => Ok(()),
        }
    }
}
