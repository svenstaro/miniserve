FROM docker.io/ubuntu
COPY --chmod=755 miniserve /app/
ENTRYPOINT ["/app/miniserve"]
