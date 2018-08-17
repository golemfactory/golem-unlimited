extern crate rand;
extern crate secp256k1;
#[macro_use]
extern crate lazy_static;
extern crate tiny_keccak;
extern crate rustc_hex;
extern crate lazycell;

use std::fmt;
use std::path::Path;
use rand::os::OsRng;
use secp256k1::Secp256k1;
use secp256k1::key::{SecretKey, PublicKey};
pub use secp256k1::{Message, Signature, Error};
use tiny_keccak::Keccak;
use rustc_hex::ToHex;
use lazycell::LazyCell;

const KEY_FILE_NAME: &str = "keystore.json";
const ADDRESS_LENGTH: usize = 20;

pub type Address = [u8; ADDRESS_LENGTH];

lazy_static! {
	static ref SECP256K1: Secp256k1 = Secp256k1::new();
}

pub trait EthKey {
    /// generates random keys: secret + public
    fn generate() -> Self;

    /// get private key
    fn private(&self) -> &SecretKey;

    /// get public key
    fn public(&self) -> &PublicKey;

    /// get ethereum address
    fn address(&self) -> &Address;

    /// signs message with sef key
    fn sign(&self, msg: Message) -> Result<Signature, Error>;

    /// verifies signature for message and self key
    fn verify(&self, msg: Message, sig: Signature) -> Result<(), Error>;

    /// ciphers given plain data
    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, Error>;

    /// deciphers given encrypted data
    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, Error>;

    /// stores keys on disk with pass
    fn serialize(&self, file_path: &Path, passwd: &str);

    /// reads keys from disk; pass needed
    fn deserialize(&self, file_path: &Path, passwd: &str);
}

#[derive(Debug)]
pub struct KeyPair {
    private: SecretKey,
    public: PublicKey,
    address: LazyCell<Address>,
}

impl EthKey for KeyPair {
    fn generate() -> Self {
        let mut rng = OsRng::new().unwrap();
        let (private, public) = SECP256K1.generate_keypair(&mut rng)
            .expect("should generate key pair");

        KeyPair { private, public, address: LazyCell::new() }
    }

    fn private(&self) -> &SecretKey {
        &self.private
    }

    fn public(&self) -> &PublicKey {
        &self.public
    }

    fn address(&self) -> &Address {
        self.address.borrow_with(|| {
            let mut hash = [0u8; 32];
            Keccak::keccak256(&self.public.serialize_vec(&SECP256K1, false), &mut hash);
            let mut result = [0u8; ADDRESS_LENGTH];
            result.copy_from_slice(&hash[12..]);
            result
        })
    }

    fn sign(&self, msg: Message) -> Result<Signature, Error> {
        SECP256K1.sign(&msg, &self.private)
    }

    fn verify(&self, msg: Message, sig: Signature) -> Result<(), Error> {
        SECP256K1.verify(&msg, &sig, &self.public)
    }

    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, Error> {
        unimplemented!("{:?}", plain)
    }

    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, Error> {
        unimplemented!("{:?}", encrypted)
    }

    fn serialize(&self, file_path: &Path, passwd: &str) {
        unimplemented!("{:?}, {:?}", file_path, passwd)
    }

    fn deserialize(&self, file_path: &Path, passwd: &str) {
        unimplemented!("{:?}, {:?}", file_path, passwd)
    }
}

impl fmt::Display for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "\n\t{:?}\n\t{:?}\n\tAddress({})",
               self.private, self.public, self.address().to_hex())
    }
}