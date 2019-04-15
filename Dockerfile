FROM rust
ARG gu_module

ENV gu_module=$gu_module

WORKDIR /usr/src/gu
COPY . .

RUN cargo install --path $gu_module 
WORKDIR /usr/src/gu/$gu_module


