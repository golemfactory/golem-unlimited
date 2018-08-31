#[macro_use]
extern crate log;
extern crate env_logger;
extern crate gu_ethkey;

use std::env;
use gu_ethkey::{KeyPair, EthKey, EthKeyStore};
//use secp256k1::Message;

fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }
    env_logger::init();

    info!("Starting app {:?}", env::args());

    let key_pair = KeyPair::generate().unwrap();
    info!("Generated key pair: {}", key_pair);
    info!("Generated private key: {:?}", key_pair.private());
    info!("Generated public key: {:?}", key_pair.public());
    info!("Generated address: {:?}", key_pair.address());

    let p = "hekllo".into();
    let path = "tmpa/a";
    key_pair.save_to_file(path, &p)
        .unwrap_or_else(|e| {warn!("writing to file {}: {}", path, e)});

    let kp = KeyPair::load_from_file("tmpa/a", &p).unwrap();
    info!("Loaded key pair: {}", kp);
    let kp = KeyPair::load_from_file("tmpb/a", &p).unwrap();
    info!("Loaded key pair from pyethereum: {}", kp);

//    let mut v = [0u8; 32];
//    v[0]=39u8;
//    v[1]=50u8;
//    let msg : Message = Message::from(v);
//    let sig = key_pair.sign(msg).unwrap();
//    info!("signature {:?} for {:?}", sig, msg);
//    assert!(key_pair.verify(msg, sig).is_ok());
}