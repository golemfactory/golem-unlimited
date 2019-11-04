#[macro_use]
extern crate log;

extern crate actix;

#[allow(unused_imports)]
#[macro_use]
extern crate actix_derive;

#[macro_use]
extern crate lazy_static;

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate error_chain;
extern crate bytes;
extern crate directories;

extern crate actix_web;
extern crate futures;
extern crate gu_actix;
extern crate gu_base;
extern crate tokio_fs;
extern crate tokio_io;
extern crate tokio_uds;

pub mod error {
    use actix::MailboxError;
    use serde_json;
    use std::io;

    error_chain!(
        foreign_links {
            Json(serde_json::Error);
            Io(io::Error);
        }

        errors {
            MailboxError(e : MailboxError){}
            ConcurrentChange{}
        }
    );

    impl From<MailboxError> for Error {
        fn from(e: MailboxError) -> Self {
            ErrorKind::MailboxError(e).into()
        }
    }
}

pub mod config;
pub mod file_storage;
pub mod http;
pub mod storage;
