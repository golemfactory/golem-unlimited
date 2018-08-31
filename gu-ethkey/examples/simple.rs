#[macro_use]
extern crate log;
extern crate env_logger;
extern crate gu_ethkey;

use std::env;
use gu_ethkey::{SafeEthKey, EthKey, EthKeyStore};

fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }
    env_logger::init();

    info!("Starting app {:?}", env::args());

    let path = "tmp/keystore.json";
    let pwd = "zimko".into();
    let key = SafeEthKey::load_or_generate(&path, &pwd).unwrap();
    info!("Generated: {}", key);
    info!("Generated public key: {:?}", key.public());
    info!("Generated address: {:?}", key.address());

    let p1 = "hekllo".into();
    if let Ok(key) = SafeEthKey::load_or_generate("tmp/pyethereum.json", &p1) {
        info!("Loaded from pyethereum: {}", key);
        key.change_password(&pwd).unwrap();
    }

//    let mut v = [0u8; 32];
//    v[0]=39u8;
//    v[1]=50u8;
//    let msg : Message = Message::from(v);
//    let sig = key.sign(msg).unwrap();
//    info!("signature {:?} for {:?}", sig, msg);
//    assert!(key.verify(msg, sig).is_ok());
}