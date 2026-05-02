FROM rust:1-slim AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .
RUN cargo build --release -p waxdemon-server

FROM debian:trixie-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 app

WORKDIR /app
COPY --from=builder /build/target/release/waxdemon /app/waxdemon
COPY --from=builder /build/crates/db/migrations /app/migrations

ENV RUST_LOG=info
ENV BIND_ADDR=0.0.0.0:3000
EXPOSE 3000
USER app

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -qO- http://127.0.0.1:3000/api/collection/sync/status >/dev/null || exit 1

ENTRYPOINT ["/app/waxdemon"]
