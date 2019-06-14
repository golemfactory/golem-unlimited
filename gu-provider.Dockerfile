FROM rust as build
COPY . /src
RUN cargo install --path /src/gu-provider/

FROM ubuntu
RUN mkdir -p /opt/provider
COPY --from=build /usr/local/cargo/bin/gu-provider /usr/bin/gu-provider

WORKDIR /opt/provider
ENV RUST_LOG=info
EXPOSE 61621
ENTRYPOINT [ "/usr/bin/gu-provider" ]
CMD [ "server", "run" ]
