extern crate gu_base;
use gu_base::daemon::DaemonProcess;
use std::{thread::sleep, time::Duration};

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    let _ = DaemonProcess::create(&args[1], &args[2]).daemonize();
    sleep(Duration::from_secs(100));
}
