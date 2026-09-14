FROM rust:1.98.1-slim AS chef

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config cmake perl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
RUN rustup target add wasm32-unknown-unknown \
    && cargo install cargo-leptos --version 0.3.7 --locked \
    && cargo install cargo-chef --version 0.1.78 --locked

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS dependencies
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json \
        --package waxdemon-server --bin waxdemon
RUN cargo chef cook --release --locked --recipe-path recipe.json \
        --package waxdemon-app --target wasm32-unknown-unknown \
        --target-dir target/front --no-default-features --features hydrate

FROM dependencies AS builder
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
