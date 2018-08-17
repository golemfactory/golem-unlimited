
# gu-hub

## Command line options

Start server 
```
$ gu-hub server
```

List lan peers

```
$ gu-hub lan list
```

Check status

```
$ gu-hub status
```

## HTTP Paths

 * ```/gu-p2p``` - p2p base path
 * ```/gu-p2p/ws``` - main websocket
 * ```/gu-p2p/c/{resource-id}``` - path for static resources transmision.
 
 
 Reservations:
 
 * ```/cli/*``` - general mapping for cli commands
 * ```/api/*``` - task API 

 