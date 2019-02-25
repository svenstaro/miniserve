# This Dockerfile results in an Alpine container containing the minishift executable.
# Use this in case you need additional basic tools provided by Alpine in this container.
FROM rustlang/rust:nightly as builder

ENV APP_HOME /usr/src/app/

RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y upx musl-tools

COPY . $APP_HOME
WORKDIR $APP_HOME
RUN make build-linux

FROM alpine
RUN apk add rsync
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/miniserve /app/

EXPOSE 8080
ENTRYPOINT ["/app/miniserve"]
