# WaxDemon

Self-hosted dashboard for your Discogs collection. Tracks collection value over time and other statistics over time.

Per-user collections with Discogs OAuth; other users require administrator approval.

> [!WARNING]
> This project is significantly AI-Supported.
> Assume there are bugs, rough edges, missing validation, and incorrect assumptions.

## Install using Helm

The chart is published as an OCI artifact to GitHub Container Registry on every
release. Install it directly — no `helm repo add` required (Helm 3.8+):

In production prefer a pre-existing Secret managed by your secrets stack:

```bash
helm install waxdemon oci://ghcr.io/rtuszik/waxdemon/waxdemon \
  --version 1.0.0 \
  --set secrets.existingSecret=waxdemon-secrets \
  --set config.PUBLIC_URL='https://waxdemon.example.com' \
  --set config.DISCOGS_USERNAME='your_handle'
```

The Secret must contain keys `DATABASE_URL`, `DISCOGS_CONSUMER_KEY`,
`DISCOGS_CONSUMER_SECRET` and `keyring.json`. See `charts/waxdemon/values.yaml`.

## Running with Docker Compose

```bash
# copy example env variables
cp .env.example .env
```

Fill in `.env` and create the keyring at `OAUTH_KEYRING_SOURCE`, then run
`docker compose up -d`.

## Running from source

Export the variables below:

```bash
mise run frontend
cargo run --release -p waxdemon-server
```

Config is done via environment variables:

| Var                                               | Required                | Purpose                                               |
| ------------------------------------------------- | ----------------------- | ----------------------------------------------------- |
| `DATABASE_URL`                                    | yes                     | Postgres connection string                            |
| `PUBLIC_URL`                                      | yes                     | Public origin; OAuth callback is `/auth/callback`     |
| `DISCOGS_CONSUMER_KEY`, `DISCOGS_CONSUMER_SECRET` | yes                     | Discogs application credentials                       |
| `DISCOGS_USERNAME`                                | until first owner login | Administrator's Discogs handle                        |
| `OAUTH_ACTIVE_KEY_ID`                             | yes                     | Active keyring ID (`primary` in Helm/Compose)         |
| `OAUTH_KEYRING_FILE`                              | yes                     | Keyring JSON file path                                |
| `BIND_ADDR`                                       | no                      | Where the HTTP server listens; default `0.0.0.0:3000` |

Keyring JSON (64-character hex keys): `{"primary":"<openssl rand -hex 32 output>"}`.
Set per-user sync intervals in Settings.

## Upgrading to 1.0.0

Stop the old version and back up the database. Configure OAuth and the keyring, keeping `DISCOGS_USERNAME` set to the existing collection owner.
Schema migrations run at startup. That owner's first OAuth login imports the
collection, history and settings and creates the administrator.
`DISCOGS_TOKEN` and `SYNC_CRON_SCHEDULE` are no longer used.

## Architecture

```
crates/
├── core         pure domain: types, currency parser, dashboard aggregation
├── db           sqlx + migrations (Postgres)
├── discogs      reqwest client: retry/backoff, pagination
├── sync         orchestrator
├── app          Leptos SSR/WASM + ECharts
└── server       Axum HTTP + OAuth + Apalis background jobs
```
