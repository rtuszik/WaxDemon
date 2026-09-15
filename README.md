# WaxDemon

Self-hosted dashboard for your Discogs collection. Tracks collection value over time and other statistics over time.

> [!WARNING]
> This project is significantly AI-Supported.
> Assume there are bugs, rough edges, missing validation, and incorrect assumptions.

## Install using Helm

The chart is published as an OCI artifact to GitHub Container Registry on every
release. Install it directly — no `helm repo add` required (Helm 3.8+):

In production prefer a pre-existing Secret managed by your secrets stack:

```bash
helm install waxdemon oci://ghcr.io/rtuszik/waxdemon/waxdemon \
  --version 1.6.0 \
  --set secrets.existingSecret=waxdemon-secrets \
  --set config.PUBLIC_URL='https://waxdemon.example.com'
```

The Secret must contain keys `DATABASE_URL`, `DISCOGS_CONSUMER_KEY`,
`DISCOGS_CONSUMER_SECRET` and `keyring.json`. See `charts/waxdemon/values.yaml`.

## Running with Docker Compose

see [docker-compose.yaml](docker-compose.yaml)

```bash
# copy example env variables
cp .env.example .env
```

Fill in `.env`, setting a database password and a matching `DATABASE_URL` using
`postgres:5432`. Create the keyring at `OAUTH_KEYRING_SOURCE`, then run
`docker compose up -d`.

## Config

Config is done via environment variables:

| Var                                               | Required | Purpose                                               |
| ------------------------------------------------- | -------- | ----------------------------------------------------- |
| `DATABASE_URL`                                    | yes      | Postgres connection string                            |
| `PUBLIC_URL`                                      | yes      | Public origin; OAuth callback is `/auth/callback`     |
| `DISCOGS_CONSUMER_KEY`, `DISCOGS_CONSUMER_SECRET` | yes      | Discogs application credentials                       |
| `OAUTH_ACTIVE_KEY_ID`                             | yes      | Active keyring ID (`primary` in Helm/Compose)         |
| `OAUTH_KEYRING_FILE`                              | yes      | Keyring JSON file path                                |
| `BIND_ADDR`                                       | no       | Where the HTTP server listens; default `0.0.0.0:3000` |

Keyring JSON (64-character hex keys): `{"primary":"<openssl rand -hex 32 output>"}`.
The first Discogs signup becomes the administrator. Later
signups require administrator approval. Set per-user sync intervals in Settings.

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
