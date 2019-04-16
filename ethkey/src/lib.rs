//! Ethereum keys management supporting keystores in formats used by [geth] (soon), [parity] and [pyethereum].
//!
//! ## Features
//!   * random key pair generation
//!   * key serialization/deserialization
//!   * keystore password change
//!   * signing and verification
//!
//! [geth]: https://github.com/ethereum/go-ethereum
//! [parity]: https://github.com/paritytech/parity-ethereum
//! [pyethereum]: https://github.com/ethereum/pyethereum
//!
//! ## Usage
//! ```toml
//! [dependencies]
//! ethkey = "0.2"
//! ```
//!
//! ## Example
//!
//! ```edition2018
//! use ethkey::prelude::*;
//!
//! fn main() {
//!     let key = EthAccount::load_or_generate("/path/to/keystore", "passwd")
//!         .expect("should load or generate new eth key");
//!
//!     println!("{:?}", key.address())
//! }
//! ```
//!

use std::{
    fmt,
    fs::File,
    num::NonZeroU32,
    path::{Path, PathBuf},
};

use ethsign::{
    keyfile::{Bytes, KeyFile},
    Protected,
};
pub use ethsign::{PublicKey, SecretKey, Signature};
use log::info;
use rand::{RngCore, thread_rng};

pub use address::Address;

mod address;

/// 32 bytes Message for signing and verification
pub type Message = [u8; 32];
/// Password. It is overwritten with zeros after memory is released.
pub type Password = Protected;

/// HMAC fn iteration count; a compromise between security and performance
pub const KEY_ITERATIONS: NonZeroU32 = unsafe { NonZeroU32::new_unchecked(10240) };
pub const KEYSTORE_VERSION: u64 = 3;

/// An Ethereum Account keys with store.
/// Allows to generate a new key pair and save it to disk as well as read existing keyfile.
/// Provides `sign` and `verify` operations for [ECC] on curve [Secp256k1].
///
/// [ECC]: https://blog.cloudflare.com/a-relatively-easy-to-understand-primer-on-elliptic-curve-cryptography/
/// [Secp256k1]: https://en.bitcoin.it/wiki/Secp256k1
pub struct EthAccount {
    secret: SecretKey,
    public: PublicKey,
    address: Address,
    file_path: PathBuf,
}

impl EthAccount {
    /// public key
    pub fn public(&self) -> &PublicKey {
        &self.public
    }

    /// Ethereum address
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// signs given message with self secret key
    pub fn sign(&self, msg: &Message) -> error::Result<Signature> {
        Ok(self.secret.sign(msg)?)
    }

    /// verifies signature for given message and self public key
    pub fn verify(&self, sig: &Signature, msg: &Message) -> error::Result<bool> {
        Ok(self.public.verify(sig, msg)?)
    }

    /// reads keys from disk or generates new ones and stores to disk; password needed
    pub fn load_or_generate<P, W>(file_path: P, password: W) -> error::Result<Box<Self>>
    where
        P: AsRef<Path>,
        W: Into<Password>,
    {
        let pwd = password.into();
        let (secret, log_msg) = match File::open(&file_path) {
            Ok(file) => {
                let key_file: KeyFile = serde_json::from_reader(file)?;
                let secret = SecretKey::from_crypto(&key_file.crypto, &pwd)?;
                (secret, "loaded")
            }
            Err(_e) => {
                let secret = SecretKey::from_raw(&random_bytes())?;
                save_key(&secret, &file_path, pwd)?;
                (secret, "generated and saved")
            }
        };

        let eth_account = EthAccount {
            address: secret.public().address().as_ref().into(),
            public: secret.public(),
            secret,
            file_path: ::std::fs::canonicalize(file_path)?,
        };

        info!("{} {}", log_msg, eth_account);

        Ok(Box::new(eth_account))
    }

    /// stores keys on disk with changed password
    pub fn change_password<W: Into<Password>>(&self, new_password: W) -> error::Result<()> {
        save_key(&self.secret, &self.file_path, new_password.into())?;
        info!("changed password for {}", self);
        Ok(())
    }
}

fn save_key<P, W>(secret: &SecretKey, file_path: &P, password: W) -> error::Result<()>
where
    P: AsRef<Path>,
    W: Into<Password>,
{
    let key_file = KeyFile {
        id: format!("{}", uuid::Uuid::new_v4()),
        version: KEYSTORE_VERSION,
        crypto: secret.to_crypto(&password.into(), KEY_ITERATIONS)?,
        address: Some(Bytes(secret.public().address().to_vec())),
    };
    serde_json::to_writer_pretty(&File::create(&file_path)?, &key_file)?;
    Ok(())
}

fn random_bytes() -> [u8; 32] {
    let mut secret = [0u8; 32];
    thread_rng().fill_bytes(&mut secret);
    secret
}

impl fmt::Display for EthAccount {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(
            fmt,
            "EthAccount address: {}, path: {:?}",
            self.address(),
            self.file_path
        )
    }
}

impl fmt::Debug for EthAccount {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        fmt.debug_struct("EthAccount")
            .field("public", &self.public)
            .field("file_path", &self.file_path)
            .finish()
    }
}

#[allow(deprecated)]
mod error {
    use error_chain::{
        error_chain, error_chain_processing, impl_error_chain_kind, impl_error_chain_processed,
        impl_extract_backtrace,
    };

    error_chain! {
        foreign_links {
            IoError(std::io::Error);
            EthsignError(ethsign::Error);
            Secp256k1Error(secp256k1::Error);
            SerdeJsonError(serde_json::Error);
        }
    }
}

pub mod prelude {
    //! A "prelude" for users of the `ethkey` crate.
    //!
    //! ```
    //! use ethkey::prelude::*;
    //! ```
    //!
    //! The prelude may grow over time.

    pub use super::{Address, EthAccount, Password, PublicKey, SecretKey, Signature};
}

#[cfg(test)]
mod tests {
    use std::{env, fs::File, path::PathBuf};

    use ethsign::keyfile::KeyFile;
    use rustc_hex::ToHex;
    use tempfile::tempdir;

    use crate::prelude::*;

    fn tmp_path() -> PathBuf {
        let mut dir = tempdir().unwrap().into_path();
        dir.push("keystore.json");
        dir
    }

    #[test]
    fn init_logging() {
        if env::var("RUST_LOG").is_err() {
            env::set_var("RUST_LOG", "info")
        }
        env_logger::init();
    }

    #[test]
    fn should_generate_save_and_load() {
        // given
        let path = tmp_path();
        let pwd = "pwd";

        // when
        let key = EthAccount::load_or_generate(&path, pwd);

        // then
        assert!(path.exists(), format!("path {:?} should exist", path));
        assert!(key.is_ok());

        // when
        let key0 = key.unwrap();
        let key1 = EthAccount::load_or_generate(&path, pwd).unwrap();

        // then
        assert_eq!(key0.address().as_ref(), key1.address().as_ref());
        assert_eq!(key0.public().bytes()[..], key1.public().bytes()[..]);
    }

    #[test]
    fn should_not_generate_when_path_points_dir() {
        // given
        let dir_path = tempdir().unwrap().into_path();

        // when
        let key = EthAccount::load_or_generate(dir_path, "pwd");

        // then
        assert!(key.is_err());
        assert_eq!(key.unwrap_err().to_string(), "Is a directory (os error 21)");
    }

    #[test]
    fn should_not_generate_when_path_permission_denied() {
        // when
        let key = EthAccount::load_or_generate("/a", "pwd");

        // then
        assert!(key.is_err());
        assert_eq!(
            key.unwrap_err().to_string(),
            "Permission denied (os error 13)"
        );
    }

    #[test]
    fn should_generate_and_serialize_with_proper_id_version_and_address() {
        // given
        let path = tmp_path();

        // when
        let key = EthAccount::load_or_generate(&path, "pwd").unwrap();

        // then
        let key_file: KeyFile = serde_json::from_reader(File::open(path).unwrap()).unwrap();

        assert_eq!(key_file.id.len(), 36);
        assert_ne!(key_file.id, "00000000-0000-0000-0000-000000000000");
        uuid::Uuid::parse_str(&key_file.id).expect("should parse as UUID");

        assert_eq!(key.address().to_vec(), key_file.address.unwrap().0);
        assert_eq!(key_file.version, 3);
    }

    #[test]
    fn should_read_keystore_generated_by_geth() {
        // when
        let key = EthAccount::load_or_generate("res/geth-keystore.json", "geth").unwrap();

        // then
        assert_eq!(
            format!("{}", key.address()),
            "0x8e049da484e853d92d118be16377ff616275d470"
        );
        assert_eq!(key.public().bytes().to_hex::<String>(), "e54553168b429c0407c5e4338f0a61fa7a515ff382ada9f323e313353c1904b0d8039f99e213778ba479196ef24c838e41dc77215c41895fe15e4de018d7d1dd");
    }

    #[test]
    fn should_read_keystore_generated_by_parity() {
        // when
        let key = EthAccount::load_or_generate("res/parity-keystore.json", "").unwrap();

        // then
        assert_eq!(
            format!("{}", key.address()),
            "0x005b3bcf82085eededd551f50de7892471ffb272"
        );
        assert_eq!(key.public().bytes().to_hex::<String>(), "782cc7dd72426893ae0d71477e41c41b03249a2b72e78eefcfe0baa9df604a8f979ab94cd23d872dac7bfa8d07d8b76b26efcbede7079f1c5cacd88fe9858f6e");
    }

    #[test]
    fn should_read_keystore_generated_by_pyethereum() {
        // when
        let key = EthAccount::load_or_generate("res/pyethereum-keystore.json", "hekloo").unwrap();

        // then
        assert_eq!(
            format!("{}", key.address()),
            "0x5240400e8b0aadfd212d9d8c70973b9800fa4b0f"
        );
        assert_eq!(key.public().bytes().to_hex::<String>(), "12e612f62a244e31c45b5bb3a99ec6c40e5a6c94d741352d3ea3aaeab71075b743ca634393f27a56f04a0ff8711227f245dab5dc8049737791b372a94a6524f3");
    }

    #[test]
    fn should_read_relative_path_as_absolute() {
        let rel_path = "res/pyethereum-keystore.json";
        let mut abs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        abs_path.push(rel_path);
        // when
        let key = EthAccount::load_or_generate(&rel_path, "hekloo").unwrap();

        // then
        assert_eq!(key.file_path, abs_path);
    }

    #[test]
    fn should_fail_generate_change_pass_and_reload_with_old_pass() {
        // given
        let path = tmp_path();
        let pwd = "zimko";

        // when
        let key = EthAccount::load_or_generate(&path, pwd);
        assert!(key.is_ok());

        // change pass
        key.unwrap().change_password("hekloo").unwrap();

        // then
        assert!(EthAccount::load_or_generate(&path, pwd).is_err());
    }

    #[test]
    fn should_generate_change_pass_and_reload_with_new_pass() {
        // given
        let path = tmp_path();

        // when
        let key = EthAccount::load_or_generate(&path, "pwd");

        // then
        assert!(key.is_ok());

        // change pass
        let pwd1 = "hekloo";
        key.unwrap().change_password(pwd1).unwrap();

        // then
        assert!(EthAccount::load_or_generate(&path, pwd1).is_ok());
    }

    #[test]
    fn should_sign_verify() {
        // given
        let msg: super::Message = rand::random::<[u8; 32]>().into();

        // when
        let key = EthAccount::load_or_generate(&tmp_path(), "pwd").unwrap();
        let sig = key.sign(&msg);

        // then
        assert!(sig.is_ok());
        let result = key.verify(&sig.unwrap(), &msg);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn should_have_display_impl() {
        let mut abs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        abs_path.push("res/pyethereum-keystore.json");
        let key = EthAccount::load_or_generate(&abs_path, "hekloo");

        assert_eq!(
            format!("{}", key.unwrap()),
            format!(
                "EthAccount \
                 address: 0x5240400e8b0aadfd212d9d8c70973b9800fa4b0f, \
                 path: {:?}",
                abs_path
            )
        );
    }

    #[test]
    fn should_have_debug_impl() {
        let mut abs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        abs_path.push("res/pyethereum-keystore.json");

        let key = EthAccount::load_or_generate(&abs_path, "hekloo");

        assert_eq!(format!("{:?}", key.unwrap()), format!("EthAccount {{ public: PublicKey {{ \
            address: \"5240400e8b0aadfd212d9d8c70973b9800fa4b0f\", \
            public: \"12e612f62a244e31c45b5bb3a99ec6c40e5a6c94d741352d3ea3aaeab71075b743ca634393f27a56f04a0ff8711227f245dab5dc8049737791b372a94a6524f3\" }}, \
            file_path: {:?} }}", abs_path));
    }
}
