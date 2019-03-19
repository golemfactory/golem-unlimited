FROM rust as build
ARG GU_BRANCH=${GU_BRANCH:-release/0.2}
RUN git clone git://github.com/golemfactory/golem-unlimited.git 
RUN cd golem-unlimited && git fetch && git checkout $GU_BRANCH
RUN cargo install --path golem-unlimited/gu-hub 

FROM ubuntu
RUN mkdir -p /opt/hub
COPY --from=build /usr/local/cargo/bin/gu-hub /usr/bin/gu-hub
COPY --from=build /golem-unlimited/gu-hub/webapp /opt/hub/webapp/
WORKDIR /opt/hub
ENV RUST_LOG=info
EXPOSE 61622
ENTRYPOINT [ "/usr/bin/gu-hub" ]
CMD [ "server", "run" ]


