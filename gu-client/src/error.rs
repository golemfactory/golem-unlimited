use failure::Fail;
use std::{error, fmt, str};
//
/// Errors returned by Rust API for Golem Unlimited
#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "invalid address: {}", _0)]
    InvalidAddress(#[fail(cause)] url::ParseError),
    #[fail(display = "invalid response: {}", _0)]
    InvalidJSONResponse(#[fail(cause)] actix_web::error::JsonPayloadError),

    #[fail(display = "resource not found")]
    ResourceNotFound,

    #[fail(display = "bad response {}", _0)]
    ResponseErr(actix_web::http::StatusCode),
    #[fail(display = "{}", _0)]
    Utf8Error(#[fail(cause)] str::Utf8Error),
    #[fail(display = "{}", _0)]
    SendRequestError(#[fail(cause)] actix_web::client::SendRequestError),
    #[fail(display = "{}", _0)]
    PayloadError(#[fail(cause)] actix_web::error::PayloadError),

    #[fail(display = "{}", _0)]
    CreateRequest(actix_web::Error),

    #[fail(display = "invalid peer {}", _0)]
    InvalidPeer(String),
}

impl From<str::Utf8Error> for Error {
    fn from(e: str::Utf8Error) -> Self {
        Error::Utf8Error(e)
    }
}

impl From<actix_web::client::SendRequestError> for Error {
    fn from(e: actix_web::client::SendRequestError) -> Self {
        Error::SendRequestError(e)
    }
}

impl From<actix_web::error::JsonPayloadError> for Error {
    fn from(e: actix_web::error::JsonPayloadError) -> Self {
        Error::InvalidJSONResponse(e)
    }
}

impl From<actix_web::error::PayloadError> for Error {
    fn from(e: actix_web::error::PayloadError) -> Self {
        Error::PayloadError(e)
    }
}
