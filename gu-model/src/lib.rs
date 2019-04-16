pub mod dockerman;
pub mod envman;

pub mod deployment;
mod hub;
pub mod peers;
pub mod session;

#[cfg(feature = "hash")]
pub mod hash;

pub type Map<K, V> = std::collections::BTreeMap<K, V>;
pub type Set<K> = std::collections::BTreeSet<K>;

pub type Tags = Set<String>;

pub use semver::Version;

pub use hub::{BuildInfo, HubInfo};

mod error {

    pub enum UpdateError {}

}

pub use chrono;
