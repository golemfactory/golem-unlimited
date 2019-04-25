extern crate vergen;

use vergen::{generate_cargo_keys, ConstantsFlags};

fn main() {
    // Setup the flags, toggling off the 'SEMVER_FROM_CARGO_PKG' flag
    let /* mut */ flags = ConstantsFlags::all();
    //flags.toggle(ConstantsFlags::BUILD_TIMESTAMP);
    //flags.toggle(ConstantsFlags::SEMVER_LIGHTWEIGHT);

    generate_cargo_keys(flags).expect("Unable to generate the cargo keys!");
}
