FROM rust as build
ARG GU_BRANCH=${GU_BRANCH:-feature/blender}
RUN git clone git://github.com/golemfactory/golem-unlimited.git 
RUN cd golem-unlimited && git fetch && git checkout $GU_BRANCH
RUN cargo install --path golem-unlimited/gu-provider

FROM ubuntu
RUN mkdir -p /opt/provider
COPY --from=build /usr/local/cargo/bin/gu-provider /usr/bin/gu-provider
WORKDIR /opt/provider
ENV RUST_LOG=info
EXPOSE 61622
ENTRYPOINT [ "/usr/bin/gu-provider" ]
CMD [ "server", "run" ]


