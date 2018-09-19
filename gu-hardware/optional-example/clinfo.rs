extern crate gu_hardware;

use gu_hardware::clinfo::*;

fn main() {
    for platform in Platforms::try_new().unwrap() {
        println!("platform: {}", platform.name());
        for device in platform.devices().unwrap() {
            println!("dev: {} vendor={}", device.name(), device.vendor());
        }
    }
}
