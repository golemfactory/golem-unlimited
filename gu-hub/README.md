
# gu-hub

Golem Unlimited management node.

Hub is a central unit within Golem Unilimited subnetwork.
[Providers](https://github.com/golemfactory/golem-unlimited/blob/release/0.2/gu-provider) are joining subnetwork
via connecting the Hub. From the other hand client apps are connecting Hub to use all resources from subordinate
providers.

Golem Unlimited integrations are build using
[Hub lo level API](http://editor.swagger.io/?url=https://raw.githubusercontent.com/golemfactory/golem-unlimited/hub-api-documented/gu-hub-api.yaml).

Hub serves Web User Interface available at http://localhost:61622/app/index.html by default. 
Golem Unlimited integrations can (but not have to) provide plugins for the Web UI.


## Build from src

Currently we support Hub on Debian based Linux distributions only.
To perform build you need working [Rust](https://rustup.rs).
Clone the repo, go to the `gu-hub` dir and run:
```
cargo build --release
```

## Running

To run the Hub server invoke:
```
$ gu-hub --user server run
```
you can omit `--user` when you have administrative priviledges.

Check other commands by invoking:

```
$ gu-hub help
```

 