FROM ubuntu:16.04

RUN apt-get update
RUN apt-get install -y openssl

ADD gu-provider /opt/gu/gu-provider

RUN apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

ENTRYPOINT ["/opt/gu/gu-provider", "server", "connect", "10.30.8.179:61622"]


