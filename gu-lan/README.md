
# gu-lan

It is an internal crate for mDNS discovery. It allows
to make one-shot as well as continuous queries about instances in local
network that have _unlimited._tcp.local type.

### Modules

 * actor - trait MdnsActor and its implementations
 * codec - encoding and decoding mDNS packets
 * continuous - implementation of actor continuous actor for specific service discovery
 * rest_client - CLI module for service discovery

 