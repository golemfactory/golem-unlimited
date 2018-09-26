FROM ubuntu:16.04
RUN apt-get update && apt-get install -y openssl && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*
ADD gu-provider /opt/gu/gu-provider

ENTRYPOINT ["/opt/gu/gu-provider", "server", "connect", "10.30.8.179:61622"]


