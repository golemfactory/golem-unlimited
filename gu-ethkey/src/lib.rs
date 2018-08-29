extern crate ethkey;
extern crate ethstore;
#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;

pub use ethkey::KeyPair;
use ethkey::{Random, Generator, Secret, Public, Address, Password, Message, sign, verify_public, Signature};
use ethkey::crypto::ecies::{encrypt, decrypt};
use ethstore::{EthStore, SimpleSecretStore, SecretVaultRef};
use ethstore::accounts_dir::{KeyDirectory, RootDiskDirectory};

error_chain!{
    foreign_links {
        KeyError(ethkey::Error);
        CryptoError(ethkey::crypto::Error);
    }
    errors {
        StoreError(e: ethstore::Error) {
            display("Store error '{}'", e)
        }
    }
}

impl From<ethstore::Error> for Error {
    fn from(e: ethstore::Error) -> Self {
        ErrorKind::StoreError(e).into()
    }
}

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
    fn sign(&self, msg: &Message) -> Result<Signature>;

    /// verifies signature for message and self key
    fn verify(&self, sig: &Signature, msg: &Message) -> Result<bool>;

    /// ciphers given plain data
    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>>;

    /// deciphers given encrypted data
    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>>;
}

pub trait EthSerde {
    /// stores keys on disk with pass
    fn save_to_file(&self, store_path: &str, passwd: &Password) -> Result<()>;

    /// reads keys from disk; pass needed
    fn load_from_file(&self, store_path: &str, passwd: &Password) -> Result<KeyPair>;
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

impl EthSerde for KeyPair {
    fn save_to_file(&self, store_path: &str, passwd: &Password) -> Result<()> {
        let dir = RootDiskDirectory::create(store_path)?;
        let path = format!("{:?}", dir.path());
        let store = EthStore::open(Box::new(dir))?;
        let acc = store.insert_account(SecretVaultRef::Root, self.secret().to_owned(), passwd)?;
        info!("account 0x{:x} stored in {}", acc.address, path);
        Ok(())
    }

    fn load_from_file(&self, store_path: &str, passwd: &Password) -> Result<KeyPair>{
        unimplemented!("{:?}, {:?}", store_path, passwd)
    }
}
