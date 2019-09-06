
# Golem Unlimited Provider

A worker node for Golem Unlimited. It is subordinate to the [Hub](../gu-hub). 

## Build from src

To perform build you need working [Rust](https://rustup.rs) on your platform.

### on Ubuntu

Clone the repo, go to the `gu-provider` dir and run:
```
cargo build --release
```
### on MacOS and Windows
```
cargo build --features "async_docker/ssl gu-hardware/clinfo" --release
```
* ssl is needed when you want to utilise docker execution environment within Golem Unlimited  
* clinfo is needed for proper GPU detection

## Run

To run the Provider invoke:
```
$ gu-provider --user server run
```

you can omit `--user` when you have administrative priviledges.

Check other commands by invoking:

```
$ gu-hub help
```

## Configuration

To configure the provider run:
```
$ gu-provider --user configure
```
than select Hub you want to join (one that you trust) and save the configuration. 
