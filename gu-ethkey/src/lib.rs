//! Ethereum keys management supporting keystores in formats used by [geth], [parity] and [pyethereum].
//!
//! ## Features
//!   * keys generation
//!   * keys serialization/deserialization
//!   * keystore password change
//!   * signing/verification
//!   * encryption/decryption
//!
//! [geth]: https://github.com/ethereum/go-ethereum
//! [parity]: https://github.com/paritytech/parity-ethereum
//! [pyethereum]: https://github.com/ethereum/pyethereum
//!
//! ## Example
//!
//! ```no_run
//! extern crate gu_ethkey;
//! use gu_ethkey::prelude::*;
//!
//! fn main() {
//!     let key = SafeEthKey::load_or_generate("/tmp/path/to/keystore", &"passwd".into())
//!         .expect("should load or generate new eth key");
//!
//!     println!("{:?}", key.address())
//! }
//! ```
//!

use error_chain::{error_chain, error_chain_processing, impl_error_chain_kind, impl_error_chain_processed, impl_extract_backtrace};
use log::info;
use std::{
    fmt, fs, io,
    num::NonZeroU32,
    path::{Path, PathBuf},
};
use rand::{thread_rng, RngCore};
use rustc_hex::ToHex;

use ethsign::{PublicKey, SecretKey, Signature, Protected, keyfile::KeyFile};

mod address;
pub use address::Address;

type Message = [u8; 32];
type Password = Protected;


/// HMAC fn iteration count; a compromise between security and performance
pub const KEY_ITERATIONS: NonZeroU32 = unsafe { NonZeroU32::new_unchecked(10240)};

/// An Ethereum Keys wrapper with Store.
pub struct SafeEthKey {
    secret: SecretKey,
    public: PublicKey,
    address: Address,
    file_path: PathBuf,
}

/// Provides basic operations for Ethereum Keys on curve [Secp256k1] (see [EC]).
///
/// [EC]: https://blog.cloudflare.com/a-relatively-easy-to-understand-primer-on-elliptic-curve-cryptography/
/// [Secp256k1]: https://en.bitcoin.it/wiki/Secp256k1
pub trait EthKey {
    /// get public key
    fn public(&self) -> &PublicKey;

    /// get Ethereum address
    fn address(&self) -> &Address;

    /// signs message with sef key
    fn sign(&self, msg: &Message) -> Result<Signature>;

    /// verifies signature for message and self key
    fn verify(&self, sig: &Signature, msg: &Message) -> Result<bool>;

    /// ciphers given plain data
    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>>;

    /// deciphers given encrypted data
    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>>;
}

/// Provides basic serde for Ethereum Keys.
pub trait EthKeyStore {
    /// reads keys from disk or generates new ones and stores to disk; pass needed
    fn load_or_generate<P>(file_path: P, pwd: &Password) -> Result<Box<Self>>
    where
        P: Into<PathBuf>;
    /// stores keys on disk with changed password
    fn change_password(&self, new_pwd: &Password) -> Result<()>;
}

impl EthKey for SafeEthKey {
    fn public(&self) -> &PublicKey {
        &self.public
    }

    fn address(&self) -> &Address {
        &self.address
    }

    fn sign(&self, msg: &Message) -> Result<Signature> {
        self.secret.sign(msg).map_err(Error::from)
    }

    fn verify(&self, sig: &Signature, msg: &Message) -> Result<bool> {
        let pub_key = sig.recover(msg)?;
        Ok(pub_key.bytes()[..] == self.public.bytes()[..])
    }

    fn encrypt(&self, _plain: &[u8]) -> Result<Vec<u8>> {
//        encrypt(self.key_pair.public(), &[0u8; 0], plain).map_err(Error::from)
        unimplemented!()
    }

    fn decrypt(&self, _encrypted: &[u8]) -> Result<Vec<u8>> {
//        decrypt(self.key_pair.secret(), &[0u8; 0], encrypted).map_err(Error::from)
        unimplemented!()
    }
}

fn save_keys<P>(key: &SecretKey, pwd: &Password, file_path: &P) -> Result<()>
where
    P: AsRef<Path>,
{
    let id = format!("{}", uuid::Uuid::new_v4());
    let key_file = key.to_keyfile(id, pwd, KEY_ITERATIONS)?;
    let file = &fs::File::create(&file_path)?;
    serde_json::to_writer_pretty(file, &key_file)?;
    Ok(())
}

impl EthKeyStore for SafeEthKey {
    fn load_or_generate<P>(file_path: P, pwd: &Password) -> Result<Box<Self>>
    where
        P: Into<PathBuf>,
    {
        let file_path = file_path.into();
        match fs::File::open(&file_path).map_err(Error::from) {
            Ok(file) => {
                let key_file: KeyFile = serde_json::from_reader(file)?;
                let key = SecretKey::from_keyfile(&key_file, &pwd)?;
                info!(
                    "account {:?} loaded from {}",
                    Address::from(key.public().address().as_ref()),
                    file_path.display()
                );
                Ok::<SecretKey, Error>(key)
            }
            Err(_e) => {
                let mut rng = thread_rng();
                let mut secret = [0u8; 32];
                rng.fill_bytes(&mut secret);
                let key = SecretKey::from_raw(&secret)?;
                save_keys(&key, pwd, &file_path)?;
                info!(
                    "account {:?} generated, and saved to {}",
                    Address::from(key.public().address().as_ref()),
                    file_path.display()
                );
                Ok(key)
            }
        }
        .map(|key| {
            Box::new(SafeEthKey {
                address: key.public().address().as_ref().into(),
                public: key.public(),
                secret: key,
                file_path,
            })
        })
        .map_err(Error::from)
    }

    fn change_password(&self, new_pwd: &Password) -> Result<()> {
        save_keys(&self.secret, new_pwd, &self.file_path)?;
        info!(
            "changed password for account {} in {}",
            self.address(),
            self.file_path.display()
        );
        Ok(())
    }
}

impl fmt::Display for SafeEthKey {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(fmt, "SafeEthKey:\n\tpublic:  0x{}\n\taddress: {}",
            ToHex::to_hex::<String>(&self.public().bytes()[..]),
            self.address()
        )
    }
}

impl fmt::Debug for SafeEthKey {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        fmt.debug_struct("SafeEthKey")
            .field("public", &self.public)
            .field("file_path", &self.file_path)
            .finish()
    }
}

error_chain! {
    foreign_links {
        GenerationError(io::Error);
        KeyError(ethsign::Error);
        SecpError(secp256k1::Error);
        SerdeError(serde_json::Error);
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

    pub use super::{EthKey, EthKeyStore, SafeEthKey};
}

#[cfg(test)]
mod tests {
    use ::tempfile::tempdir;
    use crate::prelude::*;
    use std::{env, fs, io::prelude::*, path::PathBuf};
    use ethsign::keyfile::KeyFile;
    use rustc_hex::FromHex;
    use crate::Address;

    fn tmp_path() -> PathBuf {
        let mut dir = tempdir().unwrap().into_path();
        dir.push("keystore.json");
        dir
    }

    fn eq(x: &[u8], y: &[u8]) -> bool {
        x.iter().zip(y.iter()).all(|(a, b)| a == b)
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
        // when
        let path = tmp_path();
        let key = SafeEthKey::load_or_generate(&path, &"pwd".into());

        // then
        assert!(path.exists());
        assert!(key.is_ok());
    }

    #[test]
    fn should_serialize_with_proper_id_and_address() {
        use std::fs;
        // given
        let path = tmp_path();

        // when
        let key = SafeEthKey::load_or_generate(&path, &"pwd".into());

        // then
        assert!(key.is_ok());

        let file = fs::File::open(path).unwrap();
        let key_file: KeyFile = serde_json::from_reader(file).unwrap();
        let id = key_file.id;
        assert_eq!(id.len(), 36);
        assert_ne!(id, "00000000-0000-0000-0000-000000000000");
        assert_eq!(key.unwrap().address().to_vec(), key_file.address.unwrap().0);
    }

    #[test]
    fn should_read_keystore_generated_by_pyethereum() {
        // given
        let path = tmp_path();
        let mut file = fs::File::create(&path).unwrap();
        // TODO: include_str!
        let _ = file.write_all(b" \
        { \
            \"crypto\": { \
                \"cipher\": \"aes-128-ctr\", \
                \"ciphertext\": \"b269651fe8be95ebe0d1584093666e14ab0ccdf4b7e5f559e11fb330c706d39f\", \
                \"cipherparams\": { \
                    \"iv\": \"984e22c4f1616e7ccbadb9ad39441eb3\" \
                },\
                \"kdf\": \"pbkdf2\", \
                \"kdfparams\": { \
                    \"prf\": \"hmac-sha256\", \
                    \"dklen\": 32, \
                    \"c\": 1024, \
                    \"salt\": \"98f13427d0cdca8bd207a9787c49c366\"}, \
                    \"mac\": \"5f76fb358d3d47101f511d52a64ddfd731c1db3ad47fc543045fcbcb803e45aa\", \
                    \"version\": 1 \
                },\
             \"id\": \"ebb2ffec-2b00-a249-80c2-5f397e28dd2b\", \
             \"version\": 3\
        }");
        let pwd = "hekloo".into();

        // when
        let key = SafeEthKey::load_or_generate(path, &pwd);

        // then
        assert!(key.is_ok());
    }

    #[test]
    fn should_fail_generate_change_pass_and_reload_with_old_pass() {
        // given
        let path = tmp_path();
        let pwd = "zimko".into();

        // when
        let key = SafeEthKey::load_or_generate(&path, &pwd);
        assert!(key.is_ok());

        // change pass
        let pwd1 = "hekloo".into();
        key.unwrap().change_password(&pwd1).unwrap();

        // then
        assert!(SafeEthKey::load_or_generate(&path, &pwd).is_err());
    }

    #[test]
    fn should_generate_change_pass_and_reload_with_new_pass() {
        // given
        let path = tmp_path();

        // when
        let key = SafeEthKey::load_or_generate(&path, &"pwd".into());
        assert!(key.is_ok());

        // change pass
        let pwd1 = "hekloo".into();
        key.unwrap().change_password(&pwd1).unwrap();

        // then
        assert!(SafeEthKey::load_or_generate(&path, &pwd1).is_ok());
    }

    #[test]
    fn should_sign_verify() {
        // given
        let msg: super::Message = rand::random::<[u8; 32]>().into();

        // when
        let key = SafeEthKey::load_or_generate(&tmp_path(), &"pwd".into()).unwrap();
        let sig = key.sign(&msg);

        // then
        assert!(sig.is_ok());
        let result = key.verify(&sig.unwrap(), &msg);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[ignore]
    #[test]
    fn should_encrypt_decrypt() {
        // given
        let plain: [u8; 32] = rand::random();

        // when
        let key = SafeEthKey::load_or_generate(&tmp_path(), &"pwd".into()).unwrap();
        let encv = key.encrypt(&plain);

        // then
        assert!(encv.is_ok());
        assert!(eq(
            key.decrypt(&encv.unwrap().as_slice()).unwrap().as_slice(),
            &plain,
        ));
    }

    #[ignore]
    #[test]
    fn should_have_debug_impl() {
        let key = SafeEthKey::load_or_generate(&tmp_path(), &"pwd".into());

        // TODO
        assert_eq!(format!("{:?}", key), "TODO");
    }

    #[ignore]
    #[test]
    fn should_have_display_impl() {
        let key = SafeEthKey::load_or_generate(&tmp_path(), &"pwd".into());

        // TODO
        assert_eq!(format!("{:?}", key), "TODO");
    }
}
