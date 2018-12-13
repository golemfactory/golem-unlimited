extern crate actix;
extern crate futures;

pub mod flatten;
pub mod prelude;

pub use self::flatten::{FlattenFuture, FlattenResult};
