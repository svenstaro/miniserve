FROM rust as builder

ENV APP_HOME /usr/src/app/

RUN rustup default nightly-2020-07-20
RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt-get install -y upx musl-tools

COPY . $APP_HOME
WORKDIR $APP_HOME
RUN make build-linux

FROM scratch
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/miniserve /app/

EXPOSE 8080
ENTRYPOINT ["/app/miniserve"]
