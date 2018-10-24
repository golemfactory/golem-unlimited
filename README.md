# Golem Unlimited

Put your idle resources into use to perform computations you need internally or publicly rent your joint computing power for a fee.

Golem Unlimited features a [hub](gu-hub) acting as a requestor and computers in the hub’s trusted network being [providers](gu-provider).

It is meant for data center-like (render farms, or desktops within organization LAN) where network participants trust each other. This allows eliminating the economic layer, reputation, verification, and sandboxing (in contrast to public Golem Network).

Golem Unlimited is a part of the broader Golem ecosystem. The hub will be able to expose whole subnetwork acting as a [Golem](../../../golem) Network provider and earn GNTs.

# Use cases
So far we’ve prepared plugins for two use cases
* Integer factorization
* Mining 

We will open source for those plugins soon.

# Installing and testing

Please bear in mind that Golem Unlimited is in  [Alpha](https://en.wikipedia.org/wiki/Software_release_life_cycle#Alpha) stage.

## binary
To install you can use [released](../../releases) Ubuntu `deb` and MacOs `dmg` binary packages.

Detailed steps can be found in our demo https://youtu.be/J0LBdg2j6Tk

## from source
To run the hub, go to the `gu-hub` subdir and perform `cargo run -- -vv server run’

To run the provider and connect to your hub at 192.168.1.1 go to `gu-provider` subdir and run `cargo run -- -vv -a 192.168.1.1:61622 server run`

# Usage

See our demo for sample usage
https://youtu.be/J0LBdg2j6Tk


# Project layout

*  [`gu-actix`]: small util crate defining flatten trait for ActixWeb future
*  [`gu-base`]: implementation of common parts of Provider and Hub servers
*  [`gu-ethkey`]: Ethereum keys management
*  [`gu-event-bus`]: event-bus implementation - publish-subscribe communication between components
*  [`gu-hardware`]: discovery of hardware resources - GPU, disk space, RAM
*  [`gu-hub`]: binary of Hub server
*  [`gu-lan`]: mDNS services discovery
*  [`gu-net`]: network layer of the application
*  [`gu-persist`]: filesystem, persistent storage of files
*  [`gu-provider`]: binary of Provider service
*  [`gu-webapp`]: web application building development tools

[`gu-actix`]: gu-actix
[`gu-base`]: gu-base
[`gu-ethkey`]: gu-ethkey
[`gu-event-bus`]: gu-event-bus
[`gu-hardware`]: gu-hardware
[`gu-hub`]: gu-hub
[`gu-lan`]: gu-lan
[`gu-net`]: gu-net
[`gu-persist`]: gu-persist
[`gu-provider`]: gu-provider
[`gu-webapp`]: gu-webapp


# Contact and Contributing
[Here](../../wiki/Contributing) you can find information about giving us a feedback and contributing to the project.
