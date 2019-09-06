# Golem Unlimited 
[![Build Status](https://travis-ci.org/golemfactory/golem-unlimited.svg?branch=release%2F0.2)](https://travis-ci.org/golemfactory/golem-unlimited)

Golem Unlimited utilizes **trusted** heterogeneous computing resources which are part time idle. It is meant for data center-like setup (e.g., render farms, or desktops within organisation LAN) where network participants trust each other, but it will also support trusted P2P subnetworks (e.g., distributed team machines).

It features the [hub](gu-hub) acting as a requestor and additional worker nodes in the hub’s trusted network acting as [providers](gu-provider).

Trust within Golem Unlimited subnetwork allows simplifying its design and taking care of only the computation layer. Other components such as economic layers, reputation systems, verification algorithms, and sandboxing (in contrast to the public Golem Network) can be skipped altogether or implemented optionally.

Golem Unlimited joint resources can be used to perform tasks for internal requestor - the hub operator - with no fee. At the same time the hub will be able to expose its subordinate trusted providers to the public [Golem](../../../golem) Network. In such a setting hub will act as a provider and earn GNTs.  

The latter broadens [Golem](../../../golem) Network reach, because it currently supports just single machine nodes. With Golem Unlimited it would allow more complex components, such as a whole subnetworks.

# Use cases
Initially we have prepared plugins for two use cases:
* [Integer factorization](https://github.com/golemfactory/gu-int-factorization) - a "Hello World" for Golem Unlimited 
* Mining - just to showcase the Golem Unlimited, not a industry grade minig solution  

Here you can watch a short demo with above two: https://youtu.be/J0LBdg2j6Tk

There are more integrations being prepared (outside Golem Unlimited team by with our support). To list a few:
* [gumpi](https://github.com/golemfactory/gumpi) - MPI implemented on top of Golem Unlimited
* [Hoard Compiler](https://github.com/hoardexchange/HoardCompiler) - Distributed C++ compiler for Visual Studio 2017 and 2019.


# Installing and testing

Please bear in mind that Golem Unlimited is in its [Alpha](https://en.wikipedia.org/wiki/Software_release_life_cycle#Alpha) stage.

## Hub
Currently we support Hub on Debian based Linux distributions only.

To install Hub you can use the [released](https://github.com/golemfactory/golem-unlimited/releases) Ubuntu `deb`.

### from source
To run Hub on other OS Plaforms refer the [Hub README](gu-hub).

## Provider

To install Provider you can use the [released](https://github.com/golemfactory/golem-unlimited/releases) Ubuntu `deb`
and MacOs `dmg` binary packages. There are also pre-released Provider for Windows builds (`exe`)

To install you can follow steps shown in our demo https://youtu.be/J0LBdg2j6Tk

### from source
See the [Provider README](gu-provider) for build instructions.

# Usage

Both [hub](gu-hub) and [provider](gu-provider) can be configured via CLI. Invoke them with `help` command to see what's possible.

The [hub](gu-hub) comes also with web UI at:
```
http://<hub-ip>:61622/app/index.html
```

# Project layout

*  [`ethkey`]: Ethereum keys management
*  [`gu-actix`]: small util crate defining flatten trait for ActixWeb future
*  [`gu-base`]: implementation of common parts of Provider and Hub servers
*  [`gu-event-bus`]: event-bus implementation - publish-subscribe communication between components
*  [`gu-hardware`]: discovery of hardware resources - GPU, disk space, RAM
*  [`gu-hub`]: binary of Hub server
*  [`gu-lan`]: mDNS services discovery
*  [`gu-net`]: network layer of the application
*  [`gu-persist`]: filesystem, persistent storage of files
*  [`gu-provider`]: binary of Provider service
*  [`gu-envman-api`]: data structures used in communication with execution enviroment component on provider side.

[`gu-actix`]: gu-actix
[`gu-base`]: gu-base
[`ethkey`]: ethkey
[`gu-event-bus`]: gu-event-bus
[`gu-hardware`]: gu-hardware
[`gu-hub`]: gu-hub
[`gu-lan`]: gu-lan
[`gu-net`]: gu-net
[`gu-persist`]: gu-persist
[`gu-provider`]: gu-provider
[`gu-envman-api`]: gu-envman-api

# How to Contribute to Unlimited
[Here](CONTRIBUTING.md) you can find information in order to give us feedback  and contribute to the project.

