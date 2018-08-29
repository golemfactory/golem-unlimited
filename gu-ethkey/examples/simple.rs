#[macro_use]
extern crate log;
extern crate env_logger;
extern crate gu_ethkey;

use std::env;
use gu_ethkey::{KeyPair, EthKey};
//use secp256k1::Message;

fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }
    env_logger::init();

    info!("Starting app {:?}", env::args());

    let keys : KeyPair = KeyPair::generate();
    info!("Generated keys: {}", keys);
    info!("Generated private key: {:?}", keys.private());
    info!("Generated public key: {:?}", keys.public());
    info!("Generated address: {:?}", keys.address());
    let p = "hekllo".into();
    keys.serialize("tmp/", &p);

//    let mut v = [0u8; 32];
//    v[0]=39u8;
//    v[1]=50u8;
//    let msg : Message = Message::from(v);
//    let sig = keys.sign(msg).unwrap();
//    info!("signature {:?} for {:?}", sig, msg);
//    assert!(keys.verify(msg, sig).is_ok());
}