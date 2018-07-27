extern crate env_logger;
#[macro_use]
extern crate log;
extern crate rand;
extern crate secp256k1;

use std::{env};
use rand::os::OsRng;
use secp256k1::Secp256k1;
use secp256k1::key::{SecretKey, PublicKey};


// TODO: remove this file; it's only for test purposes
fn main() {
    env_logger::init();

    error!("Starting app {:?}", env::args());

    let mut rng = OsRng::new().unwrap();

    error!("Finishing app {:?}", rng.generate());
}

pub trait KeyGenerator {
    fn generate(&mut self) -> (SecretKey, PublicKey);
}

impl KeyGenerator for OsRng {
    fn generate(&mut self) -> (SecretKey, PublicKey) {
        let (sec, publ) = Secp256k1::new().generate_keypair(self)
            .expect("context always created with full capabilities; qed");

        (sec, publ)
    }
}