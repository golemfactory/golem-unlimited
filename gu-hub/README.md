
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

 * ```/gu-net``` - net base path
 * ```/gu-net/ws``` - main websocket
 * ```/gu-net/c/{resource-id}``` - path for static resources transmision.
 
 
 Reservations:
 
 * ```/cli/*``` - general mapping for cli commands
 * ```/api/*``` - task API 

 