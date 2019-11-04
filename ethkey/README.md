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

```toml
[dependencies]
ethkey = "0.3"
```

## Example
(Rust edition 2018)
```rust
use ethkey::prelude::*;

fn main() {
    let key = EthAccount::load_or_generate("/tmp/path/to/keystore", "passwd")
        .expect("should load or generate new eth key");

    println!("{:?}", key.address());

    let message = [7_u8; 32];

    // sign the message
    let signature = key.sign(&message).unwrap();

    // verify the signature
    let result = key.verify(&signature, &message).unwrap();
    println!("{}", if result {"verification ok"} else {"wrong signature"});
}
```