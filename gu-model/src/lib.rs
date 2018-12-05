extern crate actix;
#[macro_use]
extern crate serde_derive;
extern crate actix_web;
extern crate gu_net;
extern crate serde;
#[cfg(test)]
extern crate serde_json;

pub mod envman;

pub mod deployment;
pub mod peers;
pub mod session;

type Map<K, V> = std::collections::BTreeMap<K, V>;
type Set<K> = std::collections::BTreeSet<K>;

pub type Tags = Set<String>;
