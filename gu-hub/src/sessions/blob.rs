use super::responses::*;
use actix_web::{dev::Payload, fs::NamedFile};
use futures::Future;
use std::{fs, fs::File, io, path::PathBuf};
use gu_base::files::write_async_with_sha1;
use actix_web::http::header::HeaderValue;

#[derive(Clone, Debug)]
pub struct Blob {
    path: PathBuf,
    sent: bool,
    etag: HeaderValue,
}

impl Blob {
    pub fn new(path: PathBuf) -> io::Result<Blob> {
        File::create(&path)?;

        Ok(Blob { path, sent: false, etag: HeaderValue::from_static("") })
    }

    pub fn write(mut self, fut: Payload) -> impl Future<Item = Blob, Error = SessionErr> {
        write_async_with_sha1(fut, self.path.clone())
            .map_err(|e| SessionErr::FileError(e))
            .and_then(move |sha| {
                self.etag = HeaderValue::from_str(&sha).map_err(|_| SessionErr::FileError(format!("Invalid file sha1 checksum: {}", sha)))?;
                self.sent = true;
                Ok(self)
            })
    }

    pub fn read(self) -> Result<(NamedFile, HeaderValue), SessionErr> {
        if !self.sent {
            return Err(SessionErr::BlobLockedError);
        }

        NamedFile::open(&self.path)
            .map_err(|e| SessionErr::FileError(e.to_string()))
            .map(|f| (f, self.etag))
    }

    pub fn clean_file(&self) -> io::Result<()> {
        match (&self.path).exists() {
            true => fs::remove_file(&self.path),
            false => Ok(()),
        }
    }
}
