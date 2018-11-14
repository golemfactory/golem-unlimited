extern crate gu_base;
use gu_base::daemon::DaemonProcess;
use std::{thread::sleep, time::Duration};

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    let p = DaemonProcess::create(&args[1], &args[2]);
    let _ = p.daemonize();
    println!("working");
    sleep(Duration::from_secs(3));
    println!("still working");
    sleep(Duration::from_secs(100));
}
