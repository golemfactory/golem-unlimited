FROM ubuntu:16.04
RUN apt-get update && apt-get install -y openssl && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*
ADD webapp /opt/gu/webapp
ADD gu-hub /opt/gu/gu-hub
RUN mkdir -p /root/.local/share/golemunlimited/
WORKDIR /opt/gu

ENTRYPOINT ["./gu-hub", "server", "run"]
