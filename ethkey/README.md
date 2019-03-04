# EthKey
Ethereum keys management supporting keystores in formats used by
[geth](https://github.com/ethereum/go-ethereum),
[parity](https://github.com/paritytech/parity-ethereum)
and
[pyethereum](https://github.com/ethereum/pyethereum).

## Features
  * random key pair generation
  * key serialization/deserialization
  * keystore password change
  * signing and verification

## Usage
Add this to your `Cargo.toml`:

```
[dependencies]
ethkey = "0.2"
```

## Example
```
use ethkey::prelude::*;
fn main() {
    let key = EthAccount::load_or_generate("/path/to/keystore", &"passwd".into())
        .expect("should load or generate new eth key");

    println!("{:?}", key.address())
}
```