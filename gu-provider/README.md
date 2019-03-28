
# gu-provider

Golem Unlimited worker node.

## Install from src

Clone the repo, go to the `gu-provider` dir and
```
cargo build
```
### on MacOS
ssl is needed in order to communicate with docker 
```
cargo build --features async_docker/ssl
```


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
