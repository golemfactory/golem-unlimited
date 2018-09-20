extern crate gu_hardware;

use gu_hardware::gpuinfo::gpu_count;

fn main() {
    println!("c={:?}", gpu_count().unwrap())
}
