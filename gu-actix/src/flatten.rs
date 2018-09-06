use futures::{future, Async};

pub trait FlattenResult<T, E> {
    fn flatten_result(self) -> Result<T, E>;
}

impl<T, E, TE> FlattenResult<T, E> for Result<Result<T, E>, TE>
where
    TE: Into<E>,
{
    fn flatten_result(self) -> Result<T, E> {
        match self {
            Err(e) => Err(e.into()),
            Ok(r) => r,
        }
    }
}

pub trait FlattenFuture<T, E> {
    type Future: future::Future<Item = T, Error = E>;

    fn flatten_fut(self) -> Self::Future;
}

pub struct FlatFut<F: future::Future> {
    inner: F,
}

impl<T, TE, E, F: future::Future<Item = Result<T, E>, Error = TE>> future::Future for FlatFut<F>
where
    TE: Into<E>,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        match self.inner.poll() {
            Err(e) => Err(e.into()),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Ok(Async::Ready(Err(e))) => Err(e),
            Ok(Async::Ready(Ok(v))) => Ok(Async::Ready(v)),
        }
    }
}

impl<T, TE, E, F> FlattenFuture<T, E> for F
where
    TE: Into<E>,
    F: future::Future<Item = Result<T, E>, Error = TE>,
{
    type Future = FlatFut<F>;

    fn flatten_fut(self) -> Self::Future {
        FlatFut { inner: self }
    }
}
