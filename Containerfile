FROM debian:testing-slim
COPY --chmod=755 miniserve /app/
ENTRYPOINT ["/app/miniserve"]
