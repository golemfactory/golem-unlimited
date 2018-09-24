extern crate flate2;

use std::io::prelude::*;
use std::{env, fs, path};

fn main() {
    let mut outf = fs::OpenOptions::new()
        .read(false)
        .write(true)
        .create(true)
        .append(true)
        .open("/tmp/build.log")
        .unwrap();

    writeln!(outf, "start!");
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path: path::PathBuf = out_dir.into();

    let webapp = fs::read_dir("webapp").unwrap();

    for f in webapp {
        let entry = f.unwrap();
        writeln!(outf, "entry: {:?}", entry.path());
    }
}
