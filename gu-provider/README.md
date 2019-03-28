
# gu-provider

Golem Unlimited worker node.

## Install from src

Clone the repo, go to the `gu-provider` dir and
```
cargo build
```
### on MacOS
```
cargo run --features "async_docker/ssl gu-hardware/clinfo"
```
* ssl is needed in order to communicate with docker 
* clinfo is needed for proper GPU detection

## Command line options

Run server
```
$ gu-provider server run
```

Start server as daemon
```
$ gu-provider server start
```

Check server status
```
$ gu-provider server status
```

Stop server daemon
```
$ gu-provider server stop
```

List available lan peers

```
$ gu-porovider lan list
```
