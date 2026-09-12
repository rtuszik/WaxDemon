FROM rust:1.97-slim AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config cmake perl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
RUN rustup target add wasm32-unknown-unknown \
    && cargo install cargo-leptos --version 0.3.7 --locked
COPY . .
RUN cargo leptos build --release --lib-cargo-args=--locked --bin-cargo-args=--locked

FROM debian:trixie-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 app

WORKDIR /app
COPY --from=builder /build/target/release/waxdemon /app/waxdemon
COPY --from=builder /build/target/site /app/site

ENV RUST_LOG=info
ENV BIND_ADDR=0.0.0.0:3000
ENV LEPTOS_SITE_ROOT=/app/site
EXPOSE 3000
USER app

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl --fail --silent --max-time 4 http://127.0.0.1:3000/health/ready >/dev/null || exit 1

ENTRYPOINT ["/app/waxdemon"]
