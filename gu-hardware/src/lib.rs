#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate serde_derive;

extern crate gu_actix;
extern crate gu_base;
extern crate gu_net;
extern crate gu_persist;

extern crate actix;
extern crate actix_web;
extern crate clap;
extern crate futures;
extern crate hostname;
extern crate num_cpus;
extern crate serde;
extern crate serde_json;
extern crate sysinfo;

#[cfg(feature = "clinfo")]
extern crate cl_sys;
#[cfg(feature = "clinfo")]
extern crate smallvec;

use gu_base::Module;
use gu_net::rpc::RemotingSystemService;

pub mod actor;
mod cli;
mod disk;
pub mod gpuinfo;
mod inner_actor;
mod ram;

#[allow(deprecated)]
pub mod error {
    use actix::MailboxError;
    use std;

    error_chain! {
        foreign_links {
            IoError(std::io::Error);
            StripPrefixError(std::path::StripPrefixError);

            CLError(super::gpuinfo::ClError) #[cfg(feature="clinfo")];
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

pub struct HardwareModule {
    _inner: (),
}

pub fn module() -> HardwareModule {
    HardwareModule { _inner: () }
}

impl Module for HardwareModule {
    fn run<D: gu_base::Decorator + Clone + 'static>(&self, _decorator: D) {
        debug!("clinfo {}", cfg!(feature = "clinfo"));
        gu_base::run_once(|| {
            let _ = self::actor::HardwareActor::from_registry();
        })
    }
}
