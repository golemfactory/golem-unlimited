
# gu-hub

Golem Unlimited management node.

## Command line options

Run server
```
$ gu-hub server run
```

Start server as daemon
```
$ gu-hub server start
```

Check server status
```
$ gu-hub server status
```

Stop server daemon
```
$ gu-hub server stop
```

List connected peers

```
$ gu-hub peer list
```

List available peers

```
$ gu-hub lan list
```


## HTTP Paths

 * ```/gu-net``` - net base path
 * ```/gu-net/ws``` - main websocket
 * ```/gu-net/c/{resource-id}``` - path for static resources transmision.
 
 
 Reservations:
 
 * ```/cli/*``` - general mapping for cli commands
 * ```/api/*``` - task API 

 