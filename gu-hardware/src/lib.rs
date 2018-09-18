#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate serde_derive;

extern crate gu_actix;
extern crate gu_base;
extern crate gu_p2p;

extern crate actix;
extern crate actix_web;
extern crate clap;
extern crate futures;
extern crate serde;
extern crate serde_json;
extern crate sysinfo;

pub mod actor;
pub mod disk;
pub mod gpu;
pub mod ram;

pub mod error {
    use actix::MailboxError;
    use std;

    error_chain! {
        foreign_links {
            IoError(std::io::Error);
            StripPrefixError(std::path::StripPrefixError);
        }

        errors {
            PathMountpointNotFound(p: std::path::PathBuf) {
                description("couldn't find mount point of path")
                display("couldn't find mount point of path {:?}", p)
            }

            MailboxError(e: MailboxError) {
                description("couldn't find mount point of path")
                display("couldn't find mount point of path {:?}", e)
            }
        }
    }

    impl From<MailboxError> for Error {
        fn from(e: MailboxError) -> Self {
            ErrorKind::MailboxError(e).into()
        }
    }
}
