
pub mod flatten;
pub mod prelude;
pub mod release;
pub mod pipe;

pub use self::flatten::{FlattenFuture, FlattenResult};

pub use futures;

#[cfg(test)]
extern crate tokio_timer;

#[macro_export]
macro_rules! async_try {
    ($expr:expr) => {
        match $expr {
            ::std::result::Result::Ok(val) => val,
            ::std::result::Result::Err(err) => {
                return $crate::futures::future::Either::B(::futures::future::err(
                    ::std::convert::From::from(err),
                ))
            }
        }
    };
    ($expr:expr,) => {
        try!($expr)
    };
}

#[macro_export]
macro_rules! async_result {
    ($expr:expr) => {
        $crate::futures::future::Either::A($expr)
    };
}
