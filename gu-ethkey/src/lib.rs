extern crate ethkey;
extern crate ethstore;
#[macro_use]
extern crate log;

pub use ethkey::KeyPair;
use ethkey::{Random, Generator, Secret, Public, Address, Password, Message, sign, verify_public, Signature, Error};
use ethkey::crypto::ecies::{encrypt, decrypt};
use ethkey::crypto::Error as CryptoError;
use ethstore::{EthStore, SimpleSecretStore, SecretVaultRef, StoreAccountRef, Error as StoreError};
use ethstore::accounts_dir::{KeyDirectory, RootDiskDirectory};

pub trait EthKey {
    /// generates random keys: secret + public
    fn generate() -> Self;

    /// get private key
    fn private(&self) -> &Secret;

    /// get public key
    fn public(&self) -> &Public;

    /// get ethereum address
    fn address(&self) -> Address;

    /// signs message with sef key
    fn sign(&self, msg: &Message) -> Result<Signature, Error>;

    /// verifies signature for message and self key
    fn verify(&self, sig: &Signature, msg: &Message) -> Result<bool, Error>;

    /// ciphers given plain data
    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, CryptoError>;

    /// deciphers given encrypted data
    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, CryptoError>;

    /// stores keys on disk with pass
    fn serialize(&self, store_path: &str, passwd: &Password) -> Result<StoreAccountRef, StoreError>;

    /// reads keys from disk; pass needed
    fn deserialize(&self, store_path: &str, passwd: &Password);
}


impl EthKey for KeyPair {
    fn generate() -> Self {
        Random.generate().unwrap()
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

    fn sign(&self, msg: &Message) -> Result<Signature, Error> {
        sign(self.secret(), msg)
    }

    fn verify(&self, sig: &Signature, msg: &Message) -> Result<bool, Error> {
        verify_public(self.public(), sig, msg)
    }

    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, CryptoError> {
        encrypt(self.public(), &[0u8;0], plain)
    }

    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, CryptoError> {
        decrypt(self.private(), &[0u8;0], encrypted)
    }

    fn serialize(&self, store_path: &str, passwd: &Password) -> Result<StoreAccountRef, StoreError> {
        let dir = RootDiskDirectory::create(store_path)?;
        let path = format!("{:?}", dir.path());
        let store = EthStore::open(Box::new(dir))?;
        let acc = store.insert_account(SecretVaultRef::Root, self.secret().to_owned(), passwd)?;
        info!("account 0x{:x} stored in {}", acc.address, path);
        Ok(acc)
    }

    fn deserialize(&self, store_path: &str, passwd: &Password) {
        unimplemented!("{:?}, {:?}", store_path, passwd)
    }
}
