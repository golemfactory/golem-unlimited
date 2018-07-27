extern crate rand;
extern crate secp256k1;
#[macro_use]
extern crate lazy_static;

pub use rand::os::OsRng;
use secp256k1::Secp256k1;
use secp256k1::key::{SecretKey, PublicKey};
use secp256k1::{Message, Signature, Error};
use std::path::Path;

lazy_static! {
	static ref SECP256K1: Secp256k1 = Secp256k1::new();
}

pub const KEY_FILE_NAME: &str = "keystore.json";

#[derive(Debug)]
pub struct Keys {
    private: SecretKey,
    public: PublicKey,

}

pub trait EthKey {
    /// generates random keys: secret + public
    fn generate(&mut self);

    /// signs message with sef key
    fn sign(self, msg: Message) -> Result<Signature, Error>;

    /// verifies signature for message and self key
    fn verify(sig: Signature, msg: Message) -> Result<bool, Error>;

    /// stores keys on disk with pass
    fn serialize(self, file_path: Path, passwd: String);

    /// reads keys from disk; pass needed
    fn deserialize(self, file_path: Path, passwd: String);
}

impl EthKey for Keys {
    fn generate(&mut self) {
        let mut rng = OsRng::new().unwrap();
        let (private, public) = SECP256K1.generate_keypair(&mut rng)
            .expect("should generate key pair");
        self.private = private;
        self.public = public;
    }

    fn sign(self, message: Message) -> Result<Signature, Error> {
        unimplemented!()
    }

    fn verify(signature: Signature, message: Message) -> Result<bool, Error> {
        unimplemented!()
    }

    fn serialize(self, file_path: Path, passwd: String) {
        unimplemented!()
    }

    fn deserialize(self, file_path: Path, passwd: String) {
        unimplemented!()
    }
}

//impl Keys {
//    fn generate_keys(&mut self) {
//        (self.private, self.public) = self.generate();
//    }
//}