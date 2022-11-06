FROM gcr.io/distroless/cc
COPY --chmod=755 miniserve /app/
ENTRYPOINT ["/app/miniserve"]
