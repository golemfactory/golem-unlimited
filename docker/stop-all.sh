#! /bin/sh

docker ps | awk '$2 == "gu-prov" { print $1 }' | xargs docker stop
docker ps -a | awk '$2 == "gu-prov" { print $1 }' | xargs docker rm

