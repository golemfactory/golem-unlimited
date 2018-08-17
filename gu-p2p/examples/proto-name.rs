extern crate digest;
extern crate sha3;

use digest::Digest;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    let out = sha3::Sha3_256::digest_str(&args[1]);

    {
        let cut = &out[0..4];
        let proto_id: u32 = (cut[0] as u32) << 24u32
            | (cut[1] as u32) << 16u32
            | (cut[2] as u32) << 8u32
            | (cut[3] as u32);
        println!("a={}, proto_id={:X}", &args[1], proto_id);
    }

    println!("{:x}", out);
}
