use std::str;

use failure::Fail;

//
/// Errors returned by Rust API for Golem Unlimited
#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "invalid address: {}", _0)]
    InvalidAddress(#[fail(cause)] url::ParseError),
    #[fail(display = "invalid response: {}", _0)]
    InvalidJSONResponse(awc::error::JsonPayloadError),

    #[fail(display = "resource not found")]
    ResourceNotFound,

    #[fail(display = "bad response {}", _0)]
    ResponseErr(awc::http::StatusCode),
    #[fail(display = "{}", _0)]
    Utf8Error(#[fail(cause)] str::Utf8Error),
    #[fail(display = "{}", _0)]
    SendRequestError(String),
    #[fail(display = "{}", _0)]
    PayloadError(awc::error::PayloadError),

    #[fail(display = "{}", _0)]
    CreateRequest(awc::error::ConnectError),

    #[fail(display = "invalid peer {}", _0)]
    InvalidPeer(String),

    #[fail(display = "{}", _0)]
    Other(String),

    #[fail(display = "{}", _0)]
    IO(#[fail(cause)] std::io::Error),

    #[fail(display = "processing error")]
    ProcessingResult(Vec<String>),
}

impl From<str::Utf8Error> for Error {
    fn from(e: str::Utf8Error) -> Self {
        Error::Utf8Error(e)
    }
}

impl From<awc::error::SendRequestError> for Error {
    fn from(e: awc::error::SendRequestError) -> Self {
        Error::SendRequestError(format!("{}", e))
    }
}

impl From<awc::error::JsonPayloadError> for Error {
    fn from(e: awc::error::JsonPayloadError) -> Self {
        Error::InvalidJSONResponse(e)
    }
}

impl From<awc::error::PayloadError> for Error {
    fn from(e: awc::error::PayloadError) -> Self {
        Error::PayloadError(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IO(e)
    }
}
