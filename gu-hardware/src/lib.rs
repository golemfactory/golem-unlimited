#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

use gu_base::Module;
use gu_net::rpc::RemotingSystemService;

pub mod actor;
pub mod gpuinfo;

mod cli;
mod disk;
mod inner_actor;
mod ram;
mod storage;

pub mod error {
    use std;

    use actix::MailboxError;

    pub type Result<T> = std::result::Result<T, Error>;

    #[derive(Debug, Fail)]
    pub enum Error {
        #[fail(display = "I/O error: {}", _0)]
        Io(#[cause] std::io::Error),

        #[fail(display = "Strip path prefix error: {}", _0)]
        StripPrefix(std::path::StripPrefixError),

        #[cfg(feature = "clinfo")]
        #[fail(display = "OpenCL error: {}", _0)]
        OpenCL(super::gpuinfo::ClError),

        #[fail(display = "Couldn't find mount point of path: {:?}", _0)]
        PathMountpointNotFound(std::path::PathBuf),

        #[fail(display = "Mailbox error: {}", _0)]
        Mailbox(MailboxError),

        #[cfg(unix)]
        #[fail(display = "Nix error: {}", _0)]
        Nix(nix::Error),

        #[cfg(not(unix))]
        #[fail(display = "Storage query not supported on non-Unix OS")]
        StorageNotSupported,
    }

    macro_rules! from_def {
        ($stype:ty => $opt:ident) => {
            impl From<$stype> for Error {
                fn from(e: $stype) -> Self {
                    Error::$opt(e)
                }
            }
        };
    }

    #[cfg(feature = "clinfo")]
    from_def! { (super::gpuinfo::ClError) => OpenCL }

    impl From<MailboxError> for Error {
        fn from(e: MailboxError) -> Self {
            Error::Mailbox(e)
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
