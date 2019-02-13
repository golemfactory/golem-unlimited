use futures::prelude::*;
use gu_model::envman::Image;
use gu_model::hash::*;
use std::path::{Path, PathBuf};

struct ImageManager {}

pub enum Error {}

pub fn image(spec: Image) -> impl Future<Item = PathBuf, Error = Error> {
    // TODO: Implement this
    futures::finished("".into())
}
