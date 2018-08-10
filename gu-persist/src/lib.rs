extern crate actix;
#[macro_use]
extern crate actix_derive;

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate error_chain;
extern crate directories;

extern crate futures;
extern crate tokio_fs;
extern crate tokio_io;

pub mod error {
    use actix::MailboxError;
    use futures::future;
    use futures::Async;
    use serde_json;
    use std::io;
    use std::result as r;

    error_chain!(

    //
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

    pub trait FlattenResult<T> {
        fn flatten_result(self) -> Result<T>;
    }

    impl<T, E> FlattenResult<T> for r::Result<Result<T>, E>
    where
        E: Into<Error>,
    {
        fn flatten_result(self) -> Result<T> {
            match self {
                Err(e) => Err(e.into()),
                Ok(r) => r,
            }
        }
    }

    pub trait FlattenFuture<T> {
        type Future: future::Future<Item = T, Error = Error>;

        fn flatten_fut(self) -> Self::Future;
    }

    pub struct FlatFut<F: future::Future> {
        inner: F,
    }

    impl<T, E, F: future::Future<Item = Result<T>, Error = E>> future::Future for FlatFut<F>
    where
        E: Into<Error>,
    {
        type Item = T;
        type Error = Error;

        fn poll(&mut self) -> r::Result<Async<Self::Item>, Self::Error> {
            match self.inner.poll() {
                Err(e) => Err(e.into()),
                Ok(Async::NotReady) => Ok(Async::NotReady),
                Ok(Async::Ready(Err(e))) => Err(e),
                Ok(Async::Ready(Ok(v))) => Ok(Async::Ready(v)),
            }
        }
    }

    impl<T, E, F: future::Future<Item = Result<T>, Error = E>> FlattenFuture<T> for F
    where
        E: Into<Error>,
    {
        type Future = FlatFut<F>;

        fn flatten_fut(self) -> Self::Future {
            FlatFut { inner: self }
        }
    }

}

pub mod config;
pub mod file_storage;
pub mod storage;
