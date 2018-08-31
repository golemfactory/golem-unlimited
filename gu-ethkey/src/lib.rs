extern crate ethkey;
extern crate ethstore;
extern crate parity_crypto;
#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;

use std::path::Path;
use std::io;
use std::fs;

pub use ethkey::KeyPair;
use ethkey::{Random, Generator, Secret, Public, Address, Password, Message, sign, verify_public, Signature};
use ethkey::crypto::ecies::{encrypt, decrypt};
use ethstore::SafeAccount;
use ethstore::accounts_dir::{RootDiskDirectory, KeyFileManager, DiskKeyFileManager};

pub const KEY_ITERATIONS: u32 = 10240;

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

pub trait EthKey {
    /// generates random keys: secret + public
    fn generate() -> Result<Box<Self>>;

    /// get private key
    fn private(&self) -> &Secret;

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

pub trait EthKeyStore {
    /// stores keys on disk with pass
    fn save_to_file<P>(&self, file_path: P, pwd: &Password) -> Result<()> where P: AsRef<Path>;
    /// reads keys from disk; pass needed
    fn load_from_file<P>(file_path: P, pwd: &Password) -> Result<KeyPair> where P: AsRef<Path>;
}

impl EthKey for KeyPair {
    fn generate() -> Result<Box<Self>> {
        Random.generate().map(Box::new).map_err(Error::from)
    }

    fn private(&self) -> &Secret {
        self.secret()
    }

    fn public(&self) -> &Public {
        self.public()
    }

    fn address(&self) -> Address {
        self.address()
    }

    fn sign(&self, msg: &Message) -> Result<Signature> {
        sign(self.secret(), msg).map_err(Error::from)
    }

    fn verify(&self, sig: &Signature, msg: &Message) -> Result<bool> {
        verify_public(self.public(), sig, msg).map_err(Error::from)
    }

    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>> {
        encrypt(self.public(), &[0u8; 0], plain).map_err(Error::from)
    }

    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        decrypt(self.private(), &[0u8; 0], encrypted).map_err(Error::from)
    }
}

impl EthKeyStore for KeyPair {

    fn save_to_file<P>(&self, file_path: P, pwd: &Password) -> Result<()> where P: AsRef<Path> {
        let file_path = file_path.as_ref();
        let dir_path = file_path.parent().ok_or(ErrorKind::InvalidPath)?;
        let file_name = file_path.file_name().and_then(|n| n.to_str())
            .map(|f| f.to_owned()).ok_or(ErrorKind::InvalidPath)?;
        let dir = RootDiskDirectory::create(dir_path)?;
        let account = SafeAccount::create(
            &self, [0u8; 16], pwd,
            KEY_ITERATIONS, "".to_owned(), "{}".to_owned())?;
        let account = dir.insert_with_filename(account, file_name, false);
        info!("account 0x{:x} stored into {}", account?.address, file_path.display());
        Ok(())
    }

    fn load_from_file<P>(file_path: P, pwd: &Password) -> Result<KeyPair> where P: AsRef<Path> {
        let account = fs::File::open(&file_path)
            .map_err(Into::into)
            .and_then(|file| DiskKeyFileManager.read(None, file))?;

        let secret = account.crypto.secret(pwd)?;
        info!("account 0x{:x} read from {}", account.address, file_path.as_ref().display());
        KeyPair::from_secret(secret).map_err(Error::from)
    }
}
