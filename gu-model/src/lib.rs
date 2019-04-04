pub mod dockerman;
pub mod envman;

pub mod deployment;
pub mod peers;
pub mod session;
mod hub;

#[cfg(feature = "hash")]
pub mod hash;

pub type Map<K, V> = std::collections::BTreeMap<K, V>;
pub type Set<K> = std::collections::BTreeSet<K>;

pub type Tags = Set<String>;

pub use semver::Version;

pub use hub::{HubInfo, BuildInfo};

mod error {

    pub enum UpdateError {}

}

pub use chrono;
