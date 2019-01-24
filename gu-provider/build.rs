extern crate vergen;

use vergen::{ConstantsFlags, generate_cargo_keys, generate_version_rs};

fn main() {
    // Setup the flags, toggling off the 'SEMVER_FROM_CARGO_PKG' flag
    let mut flags = ConstantsFlags::all();
    //flags.toggle(ConstantsFlags::BUILD_TIMESTAMP);

    //flags.toggle(ConstantsFlags::SEMVER_LIGHTWEIGHT);
    

    // Generate the 'cargo:' key output
    //generate_cargo_keys(ConstantsFlags::all()).expect("Unable to generate the cargo keys!");
    generate_version_rs(flags).expect("Unable to generate constants!");
}