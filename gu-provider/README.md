
# Golem Unlimited Provider

A worker node for Golem Unlimited. It is subordinate to the [Hub](../gu-hub). 
It can be run on all three main OS Platforms.

## Build from src

To perform build you need working [Rust](https://rustup.rs) on your platform.

### on Ubuntu

Clone the repo, go to the `gu-provider` dir and run:
```
cargo build --release
```
### on macOS and Windows
```
cargo build --features "async_docker/ssl gu-hardware/clinfo" --release
```
* `ssl` is needed when you want to utilise docker execution environment within Golem Unlimited  
* `clinfo` is needed for proper GPU detection

## Run

To run the Provider invoke:
```
$ gu-provider --user server run
```

you can omit `--user` when you have administrative priviledges.

Check other commands by invoking:

```
$ gu-provider help
```

## Configuration

### GUI for Linux, macOS and Windows

Graphical user interface for Golem Unlimited Provider is an icon in the the area called menu bar on macOS, system tray on Windows or app indicator area on Linux.

| Platform | Linux | macOS | Windows |
|--|--|--|--|
| Technology | GTK, Vala | Cocoa, Swift | C#, .NET |
| Source Code | https://github.com/golemfactory/gu-provider-ui-linux/ | https://github.com/golemfactory/gu-provider-ui-mac/ | https://github.com/golemfactory/gu-provider-ui-windows/ |

Latest binaries:
https://github.com/golemfactory/golem-unlimited/releases

Before launching it on Linux, please make sure that the current user is in the golemu group. To do this, please enter:

```# adduser $USER golemu```

after installing the package and restart your computer.

To configure the provider, please right-click its icon in the menu bar or system tray and choose "Configure". A window with a list of all hubs in the local area network should be displayed.

To allow a hub to connect to your provider, please click the combo box in the "Permission" column and change it to "Allowed (Sandbox)" or "Allowed (Full Access)" (it is initially set to "Denied"). The provider will try to connect to the hub. The field in the "Status" column should change to "Pending" and then "Connected".

The provider uses mDNS to find hubs in the local area network. This does not always work (e.g. due to firewalls). To add a hub which is not recognized, please click the "Add Other Hub" button, enter IP address of the hub and click "Add".

The "Unconfigured Local Hubs" combo box can be used to set permissions for all new hubs in the local area network.

### command line configuration

To configure the provider run:
```
$ gu-provider --user configure
```
than select Hub you want to join (one that you trust) and save the configuration. 
