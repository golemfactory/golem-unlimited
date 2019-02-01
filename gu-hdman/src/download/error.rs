use failure::Fail;
use futures::sync::oneshot;
use gu_actix::safe::*;
use std::{fmt, io};

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "destination file already exists")]
    FileAlreadyExist,

    #[fail(display = "invalid track file: {}", _0)]
    InvalidTrackingFile(&'static str),

    #[fail(display = "{}", _0)]
    IoError(#[fail(cause)] io::Error),

    #[fail(display = "serialization error {}", _0)]
    Serialize(#[fail(cause)] bincode::Error),

    #[fail(display = "Overflow")]
    Overflow,

    #[fail(display = "Canceled")]
    Canceled,

    #[fail(display = "{}", _0)]
    Other(String),
}

impl From<oneshot::Canceled> for Error {
    fn from(_: oneshot::Canceled) -> Self {
        Error::Canceled
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}

impl<T: fmt::Debug + Send + Sync + 'static> From<OverflowError<T>> for Error {
    fn from(e: OverflowError<T>) -> Self {
        Error::Overflow
    }
}

impl From<bincode::Error> for Error {
    fn from(e: bincode::Error) -> Self {
        Error::Serialize(e)
    }
}

impl From<actix_web::client::SendRequestError> for Error {
    fn from(e: actix_web::client::SendRequestError) -> Self {
        use actix_web::client::SendRequestError;
        match e {
            SendRequestError::Io(e) => Error::IoError(e),
            SendRequestError::Connector(e) => Error::Other(format!("connector {}", e)),
            SendRequestError::ParseError(e) => Error::Other(format!("parse {}", e)),
            SendRequestError::Timeout => Error::Other(format!("timeout")),
        }
    }
}

impl From<actix_web::Error> for Error {
    fn from(e: actix_web::Error) -> Self {
        Error::Other(format!("{}", e))
    }
}
