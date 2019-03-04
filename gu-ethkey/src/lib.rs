//! Ethereum keys management supporting keystores in formats used by [geth], [parity] and [pyethereum].
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
//! use gu_ethkey::prelude::*;
//!
//! fn main() {
//!     let key = EthAccount::load_or_generate("/path/to/keystore", &"passwd".into())
//!         .expect("should load or generate new eth key");
//!
//!     println!("{:?}", key.address())
//! }
//! ```
//!

use error_chain::{error_chain, error_chain_processing, impl_error_chain_kind, impl_error_chain_processed, impl_extract_backtrace};
use log::info;
use std::{
    fmt, fs::File, io,
    num::NonZeroU32,
    path::{Path, PathBuf},
};
use rand::{thread_rng, RngCore};
use rustc_hex::ToHex;

use ethsign::{Protected, keyfile::{KeyFile, Bytes}};
pub use ethsign::{PublicKey, SecretKey, Signature};

mod address;
pub use address::Address;

pub type Message = [u8; 32];
pub type Password = Protected;


/// HMAC fn iteration count; a compromise between security and performance
pub const KEY_ITERATIONS: NonZeroU32 = unsafe { NonZeroU32::new_unchecked(10240)};

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
    pub fn sign(&self, msg: &Message) -> Result<Signature> {
        self.secret.sign(msg).map_err(Error::from)
    }

    /// verifies signature for given message and self public key
    pub fn verify(&self, sig: &Signature, msg: &Message) -> Result<bool> {
        let public = sig.recover(msg)?;
        Ok(public.bytes()[..] == self.public.bytes()[..])
    }

    /// reads keys from disk or generates new ones and stores to disk; password needed
    pub fn load_or_generate<P, W>(file_path: P, password: W) -> Result<Box<Self>>
    where
        P: Into<PathBuf>,
        W: Into<Password>,
    {
        let file_path = file_path.into();
        let pwd = password.into();
        match File::open(&file_path) {
            Ok(file) => {
                let key_file: KeyFile = serde_json::from_reader(file)?;
                let secret = SecretKey::from_crypto(&key_file.crypto, &pwd)?;
                Ok((secret, "loaded from"))
            }
            Err(_e) => {
                let secret = SecretKey::from_raw(&random_bytes())?;
                save_key(&secret, &file_path, pwd)?;
                Ok((secret, "generated and saved to"))
            }
        }
        .map(|(secret, log_msg)| {
            let address = Address::from(secret.public().address().as_ref());
            info!("account {:?} {} {}", address, log_msg, file_path.display());

            Box::new(EthAccount {
                address,
                public: secret.public(),
                secret,
                file_path,
            })
        })
    }

    /// stores keys on disk with changed password
    pub fn change_password<W: Into<Password>>(&self, new_password: W) -> Result<()>
    {
        save_key(&self.secret, &self.file_path, new_password.into())?;
        info!(
            "changed password for account {:?} and saved to {}",
            self.address(),
            self.file_path.display()
        );
        Ok(())
    }
}

fn save_key<P, W>(secret: &SecretKey, file_path: &P, password: W) -> Result<()>
    where
        P: AsRef<Path>,
        W: Into<Password>,
{
    let key_file = KeyFile {
        id: format!("{}", uuid::Uuid::new_v4()),
        version: 3,
        crypto: secret.to_crypto(&password.into(), KEY_ITERATIONS)?,
        address: Some(Bytes(secret.public().address().to_vec()))
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
        write!(fmt, "EthAccount:\n\tpublic:  0x{}\n\taddress: {}",
            ToHex::to_hex::<String>(&self.public().bytes()[..]),
            self.address()
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

error_chain! {
    foreign_links {
        IoError(io::Error);
        EthsignError(ethsign::Error);
        Secp256k1Error(secp256k1::Error);
        SerdeJsonError(serde_json::Error);
    }
}

pub mod prelude {
    //! A "prelude" for users of the `gu-ethkey` crate.
    //!
    //! ```
    //! use gu_ethkey::prelude::*;
    //! ```
    //!
    //! The prelude may grow over time.

    pub use super::{EthAccount, PublicKey, SecretKey, Signature, Address, Password};
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use ethsign::keyfile::KeyFile;
    use std::{env, fs::File, path::PathBuf};
    use tempfile::tempdir;

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
    fn should_generate_and_save() {
        // given
        let path = tmp_path();

        // when
        let key = EthAccount::load_or_generate(&path, "pwd");

        // then
        assert!(path.exists());
        assert!(key.is_ok());
    }

    #[test]
    fn should_serialize_with_proper_address() {
        // given
        let path = tmp_path();

        // when
        let key = EthAccount::load_or_generate(&path, "pwd");

        // then
        assert!(key.is_ok());

        let file = File::open(path).unwrap();
        let key_file: KeyFile = serde_json::from_reader(file).unwrap();

        assert_eq!(key_file.id.len(), 36);
        assert_ne!(key_file.id, "00000000-0000-0000-0000-000000000000");
        uuid::Uuid::parse_str(&key_file.id).expect("should parse as UUID");

        assert_eq!(key.unwrap().address().to_vec(), key_file.address.unwrap().0);
    }

    #[test]
    fn should_read_keystore_generated_by_pyethereum() {
        // when
        let key = EthAccount::load_or_generate("res/wallet.json", "hekloo");

        // then
        assert!(key.is_ok());
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
        let key = EthAccount::load_or_generate("res/wallet.json", "hekloo");

        assert_eq!(format!("{}", key.unwrap()), "EthAccount:\n\t\
	        public:  0x12e612f62a244e31c45b5bb3a99ec6c40e5a6c94d741352d3ea3aaeab71075b743ca634393f27a56f04a0ff8711227f245dab5dc8049737791b372a94a6524f3\n\t\
	        address: 0x5240400e8b0aadfd212d9d8c70973b9800fa4b0f");
    }

    #[test]
    fn should_have_debug_impl() {
        let key = EthAccount::load_or_generate("res/wallet.json", "hekloo");

        assert_eq!(format!("{:?}", key.unwrap()), "EthAccount { public: PublicKey { \
            address: \"5240400e8b0aadfd212d9d8c70973b9800fa4b0f\", \
            public: \"12e612f62a244e31c45b5bb3a99ec6c40e5a6c94d741352d3ea3aaeab71075b743ca634393f27a56f04a0ff8711227f245dab5dc8049737791b372a94a6524f3\" }, \
            file_path: \"res/wallet.json\" }");
    }
}
