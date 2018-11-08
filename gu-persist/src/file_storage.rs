use super::{error::*, storage};
use actix::prelude::*;
use std::path::PathBuf;

pub struct FileStorage {
    dir: PathBuf,
}

impl FileStorage {
    pub fn from_path<P: Into<PathBuf>>(path: P) -> Self {
        FileStorage { dir: path.into() }
    }

    fn key_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}.json", key))
    }
}

impl Actor for FileStorage {
    type Context = SyncContext<Self>;

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("file storage stopped");
    }
}

impl Handler<storage::Fetch> for FileStorage {
    type Result = Result<Option<Vec<u8>>>;

    fn handle(&mut self, msg: storage::Fetch, _ctx: &mut Self::Context) -> Self::Result {
        use std::{fs, io};

        let path: PathBuf = self.key_path(&msg.0);

        if !path.exists() {
            return Ok(None);
        }

        let mut f = fs::File::open(path)?;

        let meta = f.metadata()?;

        let mut buf = Vec::with_capacity(meta.len() as usize);

        io::copy(&mut f, &mut buf)?;

        let end_meta = f.metadata()?;

        if meta.modified()? != end_meta.modified()? {
            bail!(ErrorKind::ConcurrentChange)
        }

        Ok(Some(buf))
    }
}

impl Handler<storage::Put> for FileStorage {
    type Result = Result<()>;

    fn handle(&mut self, msg: storage::Put, _ctx: &mut Self::Context) -> Self::Result {
        use std::{fs, io};

        let path = self.key_path(&msg.0);

        debug!("path_buf={:?}", &path);

        if path.exists() {
            fs::remove_file(&path)?;
        }

        fs::create_dir_all(&self.dir)?;

        let mut in_cursor = io::Cursor::new(msg.1);
        let mut out_file = fs::File::create(path)?;

        io::copy(&mut in_cursor, &mut out_file)?;

        Ok(())
    }
}

#[cfg(test)]
mod test {}
