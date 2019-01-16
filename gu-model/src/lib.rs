extern crate actix;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate actix_web;
extern crate chrono;
extern crate gu_net;
extern crate serde;

pub mod dockerman;
pub mod envman;

pub mod deployment;
pub mod peers;
pub mod session;

type Map<K, V> = std::collections::BTreeMap<K, V>;
type Set<K> = std::collections::BTreeSet<K>;

pub type Tags = Set<String>;

mod error {

    pub enum UpdateError {}

}
