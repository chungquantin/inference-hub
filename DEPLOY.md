# Deploying Inference Hub

The backend (Rust API, worker, Postgres) runs on Railway; the web app runs on
Vercel. Both build from this repository's `main` branch.

```
Vercel: apps/web (Next.js) ──HTTPS──▶ Railway: api ──private network──▶ postgres (pgvector, volume)
                                      Railway: worker ──────────────────▶ postgres
```

## What runs where

| Service | Image or build | Port | Health check | Notes |
|---|---|---|---|---|
| `api` | `Dockerfile`, `IH_SERVICE=api` | `PORT` (Railway sets it) | `GET /health` | Applies migrations on start (`RUN_MIGRATIONS=true`). Public domain. |
| `worker` | `Dockerfile`, `IH_SERVICE=worker` | 9090 | `GET /healthz` | No public domain. `/readyz` and Prometheus `/metrics` on the same port. |
| `postgres` | `pgvector/pgvector:pg17` | 5432 | — | Volume at `/var/lib/postgresql/data`, 1 GB to start. Private network only. |
| web | Vercel, root `apps/web` | — | — | Server-rendered; calls the API with `API_URL`. |

The one Docker image holds the `api`, `worker` and `seed` binaries and the
curated `data/`. Migrations are compiled into the binaries.

## Railway

The project is described in [`.railway/railway.ts`](.railway/railway.ts)
(Railway Infrastructure as Code). Install the [Railway
CLI](https://docs.railway.com/cli) 5.42.1 or newer, then:

```sh
pnpm install                  # installs the `railway` SDK the file imports
railway login
railway link                  # pick or create the project and environment
railway config plan           # preview what would change
railway config apply          # apply after reviewing the plan
```

### Plan requirements

Volumes need a paid plan for real use: Railway's Trial plan allows 0.5 GB,
Hobby 5 GB, Pro 50 GB, and a volume can be resized live. The file requests
1 GB. Each volume attaches to one service and cannot be shared, so Postgres
runs as a single instance; `api` and `worker` are stateless.

### Secrets

Set these in the Railway dashboard (the file keeps them with `preserve()`, so
they never live in git):

| Service | Variable | Value |
|---|---|---|
| `postgres` | `POSTGRES_PASSWORD` | `openssl rand -hex 24` |
| `api` | `ADMIN_TOKEN` | `openssl rand -hex 32`; the same value goes to Vercel |
| `api` | `CORS_ORIGINS` | The web app's origin, such as `https://your-domain` |
| `api` | `ADMIN_ALLOWED_IPS` | Optional: networks allowed to use the admin API, such as `203.0.113.7, 2001:db8::/32`. Leave empty to rely on the token alone. |
| `worker` | `GITHUB_TOKEN` | Optional: a token with no scopes, for 5,000 GitHub requests per hour instead of 60 |
| `worker` | `LIQUID_API_KEY` | Optional: turns on LLM news triage. Spend is capped by `MAX_ENRICH_USD_PER_DAY` (1.00 in production). |

`DATABASE_URL` is built from the Postgres service's variables over Railway's
private network; you do not set it.

### First deploy

1. `railway config apply`, then wait for `postgres`, `api` and `worker` to turn
   healthy. The API and worker retry the database for up to two minutes, so
   their order does not matter.
2. Import the curated data once, inside the deployed worker container (it has
   the `seed` binary, `data/` and the private network to Postgres):

   ```sh
   railway ssh --service worker -- /app/bin/seed /app/data
   ```

   `railway ssh` needs an SSH key registered with Railway; the CLI offers to
   register one on first use.

   Run the seed again after every change under `data/`. It is idempotent.
3. Market adapters start on the worker's first scheduler pass (within a
   minute). Until the seed has run, adapters whose organization is missing
   fail with a clear error and retry on their next interval.
4. Give the `api` service a public domain (Settings → Networking), and note it
   for Vercel.

### Updates

Pushing to `main` redeploys `api` and `worker` when files in their watch paths
change (`crates/`, `migrations/`, `Cargo.*`, `Dockerfile`, `docker/`, and
`data/` for the worker). Migrations run when the new API starts; they are
additive, so the old worker keeps working until it is replaced.

Redeploying `postgres` has a short downtime: its volume can only be mounted by
one container at a time. The API answers 503 on `/health` meanwhile and the
worker's jobs retry.

If Postgres fails to start with a permission error on the volume, set
`RAILWAY_RUN_UID=0` on the `postgres` service only.

## Vercel

Import the repository in Vercel with root directory `apps/web` (framework
preset Next.js). Environment variables:

| Variable | Value |
|---|---|
| `API_URL` | The Railway API's public URL, such as `https://api-production-xxxx.up.railway.app` |
| `SITE_URL` | The site's public origin, for canonical URLs and the sitemap |
| `ADMIN_TOKEN` | The same value as the API's, to enable `/admin` |

Then add the Vercel origin to the API's `CORS_ORIGINS`.

## Operations

- **Logs.** Every service logs one JSON object per line (`LOG_FORMAT=json` in
  the image). Railway's log explorer can filter on fields such as
  `fields.adapter` or `level`.
- **Metrics.** The worker's `/metrics` serves job counts by kind and outcome,
  queue depth by status, the last success time and failure streak of every
  market adapter, and feed failure streaks. Alert on
  `ih_adapter_consecutive_failures > 3` and on
  `ih_worker_last_schedule_timestamp_seconds` falling behind.
- **Held prices.** A price outside plausible bounds, or more than 5× from the
  stored one, is held instead of published. Review them at
  `/admin/market#held`.
- **Failed jobs.** Jobs that ran out of attempts are listed on
  `/admin/market`.
- **Backups.** Enable Railway's volume backups on the `postgres` service, or
  run `pg_dump` from a machine with the TCP proxy URL on a schedule.

For a shell in a container, `railway ssh --service api`. For psql,
`railway ssh --service postgres -- psql -U inference_hub`.

## Running the production stack locally

`docker-compose.prod.yml` runs the same image and services:

```sh
cp .env.example .env.prod     # set POSTGRES_PASSWORD and ADMIN_TOKEN
docker compose -f docker-compose.prod.yml up -d --build
docker compose -f docker-compose.prod.yml --profile seed run --rm seed
curl localhost:4000/health
```
