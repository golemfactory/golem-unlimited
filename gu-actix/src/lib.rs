pub extern crate actix;
pub extern crate futures;

pub mod flatten;
pub mod prelude;
pub mod release;

pub use self::flatten::{FlattenFuture, FlattenResult};

#[cfg(test)]
extern crate tokio_timer;

#[macro_export]
macro_rules! async_try {
    ($expr:expr) => {
        match $expr {
            ::std::result::Result::Ok(val) => val,
            ::std::result::Result::Err(err) => {
                return ::futures::future::Either::B(::futures::future::err(
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
