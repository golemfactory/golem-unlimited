use futures::prelude::*;
use std::path::{PathBuf, Path};
use gu_model::envman::Image;
use gu_model::hash::*;

struct ImageManager {

}

pub enum Error {

}


pub fn image(spec : Image) -> impl Future<Item=PathBuf, Error=Error> {
    // TODO: Implement this
    futures::finished("".into())
}

