#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;

extern crate sysinfo;
extern crate actix;

#[cfg(feature="clinfo")]
extern crate cl_sys;
#[cfg(feature="clinfo")]
extern crate smallvec;


pub mod gpu;
pub mod ram;
pub mod disk;
pub mod actor;

#[cfg(feature="clinfo")]
pub mod clinfo;

pub mod error {
    use std;
    use actix::MailboxError;

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
            Mailbox(e : MailboxError)
        }
    }

    impl From<MailboxError> for Error {
        fn from(e: MailboxError) -> Self {
            ErrorKind::Mailbox(e).into()
        }
    }
}