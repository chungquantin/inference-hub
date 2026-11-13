# Deploying Inference Hub

Everything runs on Railway, built from this repository's `main` branch. Only
the web app is public; it calls the API over Railway's private network.

```
browser ──HTTPS──▶ web (Next.js) ──private network──▶ api ──▶ postgres (pgvector, volume)
                                                      worker ─▶ postgres
```

## What runs where

| Service | Image or build | Port | Health check | Notes |
|---|---|---|---|---|
| `api` | `Dockerfile`, `IH_SERVICE=api` | 4000 (`PORT`) | `GET /health` | Applies migrations on start (`RUN_MIGRATIONS=true`). Binds `::` for the private network. No public domain. |
| `worker` | `Dockerfile`, `IH_SERVICE=worker` | 9090 | `GET /healthz` | No public domain. `/readyz` and Prometheus `/metrics` on the same port. |
| `postgres` | `pgvector/pgvector:pg17` | 5432 | — | Volume at `/var/lib/postgresql/data`, 1 GB to start. Private network only. |
| `web` | `docker/web.Dockerfile` | 3000 (`PORT`) | `GET /api/health` | Next.js standalone server. Pages render on request; API responses are cached for five minutes. Public domain. |

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
runs as a single instance; `api`, `worker` and `web` are stateless.

### Secrets

Set these in the Railway dashboard (the file keeps them with `preserve()`, so
they never live in git):

| Service | Variable | Value |
|---|---|---|
| `postgres` | `POSTGRES_PASSWORD` | `openssl rand -hex 24` |
| `api` | `ADMIN_TOKEN` | `openssl rand -hex 32`; the same value goes to `web` |
| `api` | `CORS_ORIGINS` | Leave empty: the browser never calls the API. |
| `web` | `ADMIN_TOKEN` | The same value as the API's, to enable `/admin` |
| `web` | `SITE_URL` | The site's public origin, such as `https://your-domain`, for canonical URLs and the sitemap |
| `api` | `ADMIN_ALLOWED_IPS` | Optional: networks allowed to use the admin API, such as `203.0.113.7, 2001:db8::/32`. Leave empty to rely on the token alone. |
| `worker` | `RUN_SEED` | Not a secret: `true` in `.railway/railway.ts`. Imports `data/` before the worker starts. Set `false` to import by hand. |
| `worker` | `GITHUB_TOKEN` | Optional: a token with no scopes, for 5,000 GitHub requests per hour instead of 60 |
| `worker` | `LIQUID_API_KEY` | Optional: turns on LLM news triage. Spend is capped by `MAX_ENRICH_USD_PER_DAY` (1.00 in production). |

`DATABASE_URL` is built from the Postgres service's variables over Railway's
private network; you do not set it.

### First deploy

1. `railway config apply`, then wait for `postgres`, `api`, `worker` and `web` to turn
   healthy. The API and worker retry the database for up to two minutes, so
   their order does not matter.
2. The curated data imports itself: the worker has `RUN_SEED=true`, so its
   entrypoint runs the seed (`/app/bin/seed /app/data`) before the worker
   starts, on every deploy and restart. The seed waits up to two minutes for
   Postgres, applies migrations and is idempotent. If it fails, the worker
   starts anyway on the data already in the database and the log line
   `seed failed (exit N)` says so. To run it by hand:

   ```sh
   railway ssh --service worker -- /app/bin/seed /app/data
   ```

   (`railway ssh` needs an SSH key registered with Railway; the CLI offers to
   register one on first use.)
3. Market adapters start on the worker's first scheduler pass, right after the
   seed. Adapters whose organization is not in the database fail with a clear
   error and retry on their next interval, which only happens when the seed
   failed.
4. Give the `web` service a public domain (Settings → Networking → Generate
   Domain, or your own), and set `SITE_URL` on `web` to it. Leave `api` without
   one. The IaC syntax for a generated domain is not documented, so this is a
   click.

### Updates

Pushing to `main` redeploys `api`, `worker` and `web` when files in their watch
paths change (`crates/`, `migrations/`, `Cargo.*`, `Dockerfile`, `docker/`, and
`data/` for the worker; `apps/web/`, `packages/api-client/` and the pnpm files
for `web`). Migrations run when the new API starts; they are
additive, so the old worker keeps working until it is replaced. A change under
`data/` redeploys the worker, whose start imports it (`RUN_SEED`). Organization
logos are files under `apps/web/public/logos` that ship with the web app;
merge data and logos in one push so the two deploy together.

Redeploying `postgres` has a short downtime: its volume can only be mounted by
one container at a time. The API answers 503 on `/health` meanwhile and the
worker's jobs retry.

If Postgres fails to start with a permission error on the volume, set
`RAILWAY_RUN_UID=0` on the `postgres` service only.

## Operations

- **Logs.** Every service logs one JSON object per line (`LOG_FORMAT=json` in
  the image). Railway's log explorer can filter on fields such as
  `fields.adapter` or `level`.
- **Metrics.** The worker's `/metrics` serves job counts by kind and outcome,
  queue depth by status, the last success time and failure streak of every
  market adapter, and feed failure streaks. Alert on
  `ih_adapter_consecutive_failures > 3` and on
  `ih_worker_last_schedule_timestamp_seconds` falling behind, and on
  `ih_market_refreshed_timestamp_seconds` older than two hours (the market
  pages would be showing stale summaries).
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
curl localhost:4000/health          # the worker imported data/ on start (RUN_SEED)
open http://localhost:3000          # the web app, calling the API at http://api:4000
```
