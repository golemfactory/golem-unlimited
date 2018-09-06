#[macro_use]
extern crate error_chain;
extern crate ethkey;
extern crate ethstore;
#[macro_use]
extern crate log;
extern crate parity_crypto;
extern crate rustc_hex;

use ethkey::crypto::ecies::{decrypt, encrypt};
use ethkey::{sign, verify_public, Generator, KeyPair, Password, Random};
pub use ethkey::{Address, Message, Public, Signature};
use ethstore::accounts_dir::{DiskKeyFileManager, KeyFileManager, RootDiskDirectory};
use ethstore::SafeAccount;
use rustc_hex::ToHex;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const KEY_ITERATIONS: u32 = 10240;

/// An Ethereum `KeyPair` wrapper with Store.
pub struct SafeEthKey {
    key_pair: KeyPair,
    file_path: PathBuf,
}

/// Provides basic EC operations on curve Secp256k1.
pub trait EthKey {
    /// get public key
    fn public(&self) -> &Public;

    /// get ethereum address
    fn address(&self) -> Address;

    /// signs message with sef key
    fn sign(&self, msg: &Message) -> Result<Signature>;

    /// verifies signature for message and self key
    fn verify(&self, sig: &Signature, msg: &Message) -> Result<bool>;

    /// ciphers given plain data
    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>>;

    /// deciphers given encrypted data
    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>>;
}

/// Provides basic serde for Ethereum `KeyPair`.
pub trait EthKeyStore {
    /// reads keys from disk or generates new ones and stores to disk; pass needed
    fn load_or_generate<P>(file_path: P, pwd: &Password) -> Result<Box<Self>>
    where
        P: Into<PathBuf>;
    /// stores keys on disk with changed password
    fn change_password(&self, new_pwd: &Password) -> Result<()>;
}

impl EthKey for SafeEthKey {
    fn public(&self) -> &Public {
        self.key_pair.public()
    }

    fn address(&self) -> Address {
        self.key_pair.address()
    }

    fn sign(&self, msg: &Message) -> Result<Signature> {
        sign(self.key_pair.secret(), msg).map_err(Error::from)
    }

    fn verify(&self, sig: &Signature, msg: &Message) -> Result<bool> {
        verify_public(self.key_pair.public(), sig, msg).map_err(Error::from)
    }

    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>> {
        encrypt(self.key_pair.public(), &[0u8; 0], plain).map_err(Error::from)
    }

    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        decrypt(self.key_pair.secret(), &[0u8; 0], encrypted).map_err(Error::from)
    }
}

fn to_safe_account(key_pair: &KeyPair, pwd: &Password) -> Result<SafeAccount> {
    SafeAccount::create(
        key_pair,
        [0u8; 16],
        pwd,
        KEY_ITERATIONS,
        "".to_owned(),
        "{}".to_owned(),
    ).map_err(Error::from)
}

fn save_key_pair<P>(key_pair: &KeyPair, pwd: &Password, file_path: &P) -> Result<()>
where
    P: AsRef<Path>,
{
    let file_path = file_path.as_ref();
    let dir_path = file_path.parent().ok_or(ErrorKind::InvalidPath)?;
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|f| f.to_owned())
        .ok_or(ErrorKind::InvalidPath)?;

    let dir = RootDiskDirectory::create(dir_path)?;

    dir.insert_with_filename(to_safe_account(key_pair, pwd)?, file_name, false)
        .map(|_| ())
        .map_err(Error::from)
}

impl EthKeyStore for SafeEthKey {
    fn load_or_generate<P>(file_path: P, pwd: &Password) -> Result<Box<Self>>
    where
        P: Into<PathBuf>,
    {
        let file_path = file_path.into();
        match fs::File::open(&file_path).map_err(Error::from) {
            Ok(file) => {
                let safe_account = DiskKeyFileManager.read(None, file)?;
                let key_pair = KeyPair::from_secret(safe_account.crypto.secret(pwd)?)?;
                info!(
                    "account 0x{:x} loaded from {}",
                    key_pair.address(),
                    file_path.display()
                );
                Ok(key_pair)
            }
            Err(e) => {
                info!(
                    "Will generate new keys: file {} reading error: {}",
                    file_path.display(),
                    e
                );
                match Random.generate() {
                    Ok(key_pair) => {
                        save_key_pair(&key_pair, pwd, &file_path)?;
                        info!(
                            "new account 0x{:x} generated and stored into {}",
                            key_pair.address(),
                            file_path.display()
                        );
                        Ok(key_pair)
                    }
                    Err(e) => Err(Error::from(e)),
                }
            }
        }.map(|key_pair| {
            Box::new(SafeEthKey {
                key_pair,
                file_path,
            })
        })
            .map_err(Error::from)
    }

    fn change_password(&self, new_pwd: &Password) -> Result<()> {
        save_key_pair(&self.key_pair, new_pwd, &self.file_path)?;
        info!(
            "changed password for account 0x{:x} in {}",
            self.key_pair.address(),
            self.file_path.display()
        );
        Ok(())
    }
}

impl fmt::Display for SafeEthKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(
            f,
            "SafeEthKey:\n\tpublic:  0x{}\n\taddress: 0x{}",
            self.public().to_hex(),
            self.address().to_hex()
        )
    }
}

error_chain! {
    foreign_links {
        GenerationError(io::Error);
        KeyError(ethkey::Error);
        CryptoError(ethkey::crypto::Error);
        StoreCryptoError(parity_crypto::Error);
    }
    errors {
        StoreError(e: ethstore::Error) {
            display("Store error '{}'", e)
        }
        InvalidPath {}
    }
}

impl From<ethstore::Error> for Error {
    fn from(e: ethstore::Error) -> Self {
        ErrorKind::StoreError(e).into()
    }
}

#[cfg(test)]
mod tests {
    extern crate env_logger;
    extern crate log;
    extern crate tempfile;
    use self::tempfile::tempdir;
    use super::{EthKey, EthKeyStore, Message, SafeEthKey};
    use std::env;
    use std::fs;
    use std::io::prelude::*;
    use std::path::PathBuf;

    fn temp_keystore_path() -> PathBuf {
        let mut dir = tempdir().unwrap().into_path();
        dir.push("keystore.json");
        dir
    }

    fn eq(x: &[u8], y: &[u8]) -> bool {
        x.iter().zip(y.iter()).all(|(a, b)| a == b)
    }

    #[test]
    fn test_init_logging() {
        if env::var("RUST_LOG").is_err() {
            env::set_var("RUST_LOG", "info")
        }
        env_logger::init();
    }

    #[test]
    fn test_generate() {
        // given
        let path = temp_keystore_path();
        let pwd = "zimko".into();
        // when
        let key = SafeEthKey::load_or_generate(path, &pwd);
        // then
        assert!(key.is_ok());
    }

    #[test]
    fn test_read_keystore_generated_by_pyethereum() {
        // given
        let path = temp_keystore_path();
        let mut file = fs::File::create(&path).unwrap();
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
    fn test_generate_change_pass_and_reload_with_old_pass_should_fail() {
        // given
        let path = temp_keystore_path();
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
    fn test_generate_change_pass_and_reload_with_now_pass_should_pass() {
        // given
        let path = temp_keystore_path();
        let pwd = "zimko".into();
        // when
        let key = SafeEthKey::load_or_generate(&path, &pwd);
        assert!(key.is_ok());

        // change pass
        let pwd1 = "hekloo".into();
        key.unwrap().change_password(&pwd1).unwrap();

        // then
        assert!(SafeEthKey::load_or_generate(&path, &pwd1).is_ok());
    }

    #[test]
    fn test_sign_verify() {
        // given
        let path = temp_keystore_path();
        let pwd = "zimko".into();
        let mut v = [0u8; 32];
        v[0] = 39u8;
        v[1] = 50u8;
        let msg: Message = Message::from(v);

        // when
        let key = SafeEthKey::load_or_generate(&path, &pwd).unwrap();
        let sig = key.sign(&msg);

        // then
        assert!(sig.is_ok());
        assert!(key.verify(&sig.unwrap(), &msg).is_ok());
    }

    #[test]
    fn test_encrypt_decrypt() {
        // given
        let path = temp_keystore_path();
        let pwd = "zimko".into();
        let mut v = [0u8; 32];
        v[0] = 39u8;
        v[1] = 50u8;

        // when
        let key = SafeEthKey::load_or_generate(&path, &pwd).unwrap();
        let encv = key.encrypt(&v);

        // then
        assert!(encv.is_ok());
        assert!(eq(
            key.decrypt(&encv.unwrap().as_slice()).unwrap().as_slice(),
            &v
        ));
    }

}
