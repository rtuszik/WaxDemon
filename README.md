# WaxDemon

Self-hosted dashboard for your Discogs collection. Tracks collection value over time and other statistics over time.

Zero NodeJS.

> [!WARNING]
> This project is significantly AI-Supported.
> Assume there are bugs, rough edges, missing validation, and incorrect assumptions.

## Install using Helm

The chart is published as an OCI artifact to GitHub Container Registry on every
release. Install it directly — no `helm repo add` required (Helm 3.8+):

```bash
helm install waxdemon oci://ghcr.io/rtuszik/waxdemon/waxdemon \
  --version 0.1.0 \
  --set secrets.databaseUrl='postgres://user:pass@host:5432/waxdemon' \
  --set secrets.discogsToken='your_discogs_token' \
  --set config.DISCOGS_USERNAME='your_handle'
```

Pick a version from the [releases page](https://github.com/rtuszik/WaxDemon/releases).

In production prefer a pre-existing Secret managed by your secrets stack:

```bash
helm install waxdemon oci://ghcr.io/rtuszik/waxdemon/waxdemon \
  --version 0.1.0 \
  --set secrets.existingSecret=waxdemon-secrets \
  --set config.DISCOGS_USERNAME='your_handle'
```

To install from the working tree (e.g., when iterating on the chart locally),
point `helm install` at `./charts/waxdemon` instead of the OCI URL.

The Secret must contain keys `DATABASE_URL` and (optionally) `DISCOGS_TOKEN`. See
`charts/waxdemon/values.yaml` for ingress, HTTPRoute, autoscaling, probe, and
resource knobs.

## Running with Docker Compose

```bash
# copy example env variables
cp .env.example .env

docker compose up -d
```

## Running from source

```bash
export DATABASE_URL=postgres://user:pass@host/waxdemon
export DISCOGS_USERNAME=your_handle
export DISCOGS_TOKEN=your_token
# Optional: SYNC_CRON_SCHEDULE, 6-field cron (sec min hour dom mon dow)
cargo run --release -p waxdemon-server
```

Config is done via environment variables:

| Var                  | Required | Default        | Purpose                       |
| -------------------- | -------- | -------------- | ----------------------------- |
| `DATABASE_URL`       | yes      | —              | Postgres connection string    |
| `DISCOGS_USERNAME`   | for sync | —              | Your Discogs handle           |
| `DISCOGS_TOKEN`      | for sync | —              | Personal access token         |
| `SYNC_CRON_SCHEDULE` | no       | `0 0 0 * * *`  | Schedule for automatic syncs  |
| `BIND_ADDR`          | no       | `0.0.0.0:3000` | Where the HTTP server listens |

## Architecture

```
crates/
├── core         pure domain: types, currency parser, dashboard aggregation
├── db           sqlx + migrations (Postgres)
├── discogs      reqwest client: retry/backoff, pagination
├── sync         orchestrator
├── scheduler    tokio-cron-scheduler driving periodic syncs
└── server       Axum HTTP + Leptos SSR views + apexcharts
```

## Testing

Unit tests run without any infra:

```bash
cargo test --workspace
```

DB and end-to-end tests run against a Postgres pointed at by `TEST_DATABASE_URL`:

```bash
TEST_DATABASE_URL=postgres://ddtest:ddtest@localhost:5432/waxdemon_test \
  cargo test --workspace -- --test-threads=1
```
