#[macro_use]
extern crate log;
extern crate rustc_hex;
#[macro_use]
extern crate error_chain;

extern crate ethkey;
extern crate ethstore;
extern crate parity_crypto;

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use rustc_hex::ToHex;

use ethkey::{KeyPair, Random, Generator, Public, Address, Password, Message, sign, verify_public, Signature};
use ethkey::crypto::ecies::{encrypt, decrypt};
use ethstore::SafeAccount;
use ethstore::accounts_dir::{RootDiskDirectory, KeyFileManager, DiskKeyFileManager};

pub const KEY_ITERATIONS: u32 = 10240;

/// An Ethereum `KeyPair` wrapper with Store.
pub struct SafeEthKey{
    key_pair: KeyPair,
    file_path: PathBuf
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
        where P: Into<PathBuf>;
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
        key_pair, [0u8; 16], pwd,
        KEY_ITERATIONS, "".to_owned(), "{}".to_owned()
    ).map_err(Error::from)
}

fn save_key_pair<P>(key_pair: &KeyPair, pwd: &Password, file_path: &P) -> Result<SafeAccount>
        where P: AsRef<Path> {
    let file_path = file_path.as_ref();
    let dir_path = file_path.parent().ok_or(ErrorKind::InvalidPath)?;
    let file_name = file_path.file_name().and_then(|n| n.to_str())
        .map(|f| f.to_owned()).ok_or(ErrorKind::InvalidPath)?;

    let dir = RootDiskDirectory::create(dir_path)?;

    dir.insert_with_filename(to_safe_account(key_pair, pwd)?, file_name, false).map_err(Error::from)
}


impl EthKeyStore for SafeEthKey {
    fn load_or_generate<P>(file_path: P, pwd: &Password) -> Result<Box<Self>> where P: Into<PathBuf> {
        let file_path = file_path.into();
        fs::File::open(&file_path).map_err(Error::from)
            .and_then(|file| DiskKeyFileManager.read(None, file).map_err(Error::from))
            .and_then( |safe_account| {
                // TODO: fixme: generates new acc when pass do not match
                match KeyPair::from_secret(safe_account.crypto.secret(pwd)?) {
                    Ok(key_pair) => {
                        info!("account 0x{:x} loaded from {}", key_pair.address(), file_path.display());
                        Ok(key_pair)
                    }
                    Err(e) => Err(Error::from(e))
                }
            })
            .or_else(|_| {
                match Random.generate() {
                    Ok(key_pair) => {
                        let _ = save_key_pair(&key_pair, pwd, &file_path)?;
                        info!("new account 0x{:x} generated and stored into {}", key_pair.address(), file_path.display());
                        Ok(key_pair)
                    }
                    Err(e) => Err(Error::from(e))
                }
            })
            .map(|key_pair| Box::new(SafeEthKey {key_pair, file_path}))
    }

    fn change_password(&self, new_pwd: &Password) -> Result<()> {
        save_key_pair(&self.key_pair, new_pwd, &self.file_path)?;
        info!("password for account 0x{:x} changed. Stored into {}", self.key_pair.address(), self.file_path.display());
        Ok(())
    }
}

impl fmt::Display for SafeEthKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        write!(f, "SafeEthKey:\n\tpublic:  0x{}\n\taddress: 0x{}", self.public().to_hex(), self.address().to_hex())
    }
}

error_chain!{
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