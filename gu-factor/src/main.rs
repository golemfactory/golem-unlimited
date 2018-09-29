extern crate clap;
extern crate num_bigint;
extern crate num_cpus;
extern crate num_traits;

use num_bigint::BigUint;
use num_traits::{cast::FromPrimitive, One, Zero};
use std::cmp;
use std::str::FromStr;
use std::thread;

fn main() {
    use clap::{App, Arg};
    let m = App::new("Integer Factoring")
        .about("Factors given integer using all available CPU cores")
        .arg(Arg::with_name("number").index(1).required(true))
        .arg(Arg::with_name("from").index(2))
        .arg(Arg::with_name("to").index(3))
        .get_matches();

    let n = into(m.value_of("number").unwrap());
    let from = into(m.value_of("from").unwrap_or("1"));
    let to = into(m.value_of("to").unwrap_or(&n.to_string()));
    println!("factors of {}: {:?}", n, multithreaded_factor(n.clone(), from, to));
}

fn into(s: &str) -> BigUint {
    println!("into {}", s);
    BigUint::from_str(s.into()).unwrap()
}

fn factor(n: BigUint, from: BigUint, to: BigUint) -> Vec<BigUint> {
    let mut factors: Vec<BigUint> = Vec::new(); // creates a new vector for the factors of the number
    let mut i = from.clone();
    let to = cmp::min(n.clone(), to);
    while &i <= &to {
        if &n % &i == BigUint::zero() {
            println!("factor found: {}", i);
            factors.push(i.clone());
        }

        if &i % &BigUint::from(1000000u64) == BigUint::zero() {
            println!("i={}", &i);
        }
        i += BigUint::one();
    }
    //factors.sort(); // sorts the factors into numerical order for viewing purposes
    factors // returns the factors
}

fn multithreaded_factor(n: BigUint, from: BigUint, to: BigUint) -> Vec<BigUint> {
    // TODO: edge cases
    let workers_cnt = num_cpus::get();
    println!("using {} threads", workers_cnt);

    let to = cmp::min(n.clone(), to);
    let step = (to - from.clone()) / BigUint::from_usize(workers_cnt).unwrap();
    let mut from = from.clone();
    let mut to = from.clone() + step.clone();

    let mut factors: Vec<BigUint> = Vec::new(); // creates a new vector for the factors of the number
    let mut workers = Vec::new();
    for _i in 1..workers_cnt {
        let nc = n.clone();
        let fromc = from.clone();
        let toc = to.clone();
        workers.push(thread::spawn(move || {
            factor(nc, fromc, toc)
        }));
        from = to.clone() + BigUint::one();
        to += step.clone();
    }
    for worker in workers {
        let result = worker.join().expect("waiting for worker");
        factors.extend(result.into_iter());
    }
    factors
}
