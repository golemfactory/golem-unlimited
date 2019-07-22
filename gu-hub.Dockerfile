FROM rust as build
COPY . /src
RUN cargo install --path /src/gu-hub/

FROM ubuntu
RUN mkdir -p /opt/hub
COPY --from=build /usr/local/cargo/bin/gu-hub /usr/bin/gu-hub
COPY --from=build /src/gu-hub/webapp /opt/hub/webapp/

WORKDIR /opt/hub
ENV RUST_LOG=info
EXPOSE 61622
ENTRYPOINT [ "/usr/bin/gu-hub" ]
CMD [ "server", "run" ]
