# Build stage
FROM rust:1.75-bookworm AS builder
WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release --bins

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/arche-omega-relayer /usr/local/bin/relay
COPY --from=builder /app/target/release/clap_provider /usr/local/bin/clap_provider
RUN mkdir -p /app/certs
EXPOSE 8080 9090
ENTRYPOINT ["/usr/local/bin/relay"]
