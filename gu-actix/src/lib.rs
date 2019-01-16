pub extern crate actix;
pub extern crate futures;

pub mod flatten;
pub mod prelude;
pub mod release;

pub use self::flatten::{FlattenFuture, FlattenResult};

#[cfg(test)]
extern crate tokio_timer;
