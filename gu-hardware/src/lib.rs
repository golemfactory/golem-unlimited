#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;

extern crate sysinfo;
extern crate actix;

pub mod gpu;
pub mod ram;
pub mod disk;
pub mod actor;

pub mod error {
    use std;

    error_chain! {
        foreign_links {
            IoError(std::io::Error);
        }
        errors {

        }
    }
}