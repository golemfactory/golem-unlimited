pub mod dockerman;
pub mod envman;

pub mod deployment;
pub mod peers;
pub mod session;

#[cfg(feature = "hash")]
pub mod hash;

type Map<K, V> = std::collections::BTreeMap<K, V>;
type Set<K> = std::collections::BTreeSet<K>;

pub type Tags = Set<String>;

mod error {

    pub enum UpdateError {}

}

pub use chrono;
