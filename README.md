# Inference Hub

A directory, price tracker and news site for the inference market: model providers,
LLM gateways, open-source projects, GPU clouds, hyperscalers, hardware makers and
investors.

The worker collects live prices from 27 providers, from small GPU marketplaces to
hyperscalers, and the site shows them as market-wide indexes: every model and GPU,
the cheapest offer, the median and the spread (`/market`).

## Architecture

```
apps/web  (Next.js, server-rendered)  ──HTTP──▶  crates/api  (Rust, Axum)  ──▶  Postgres
                                                  crates/worker (ingestion) ──▶  Postgres
```

| Path | What it is |
|---|---|
| `crates/core` | Domain types. They are the API contract and derive the OpenAPI schemas. |
| `crates/db` | Pool, embedded migrations, queries (sqlx). |
| `crates/api` | HTTP API. Interactive docs are at `/docs`, and the spec is at `/openapi.json`. |
| `crates/worker` | Scheduler and job runners on a Postgres queue. Fetches feeds on their interval. |
| `crates/ingest` | Provider adapters, feed parsing, URL canonicalization, organization tagging, GitHub sync. |
| `crates/seed` | Validates and imports the curated YAML in `data/`; exports and restores the JSON archive. |
| `data/organizations/` | Curated data: one YAML file per organization, with its products, prices, feeds and tagging rules. |
| `data/sources.yaml` | Feeds not owned by one organization, such as newsletters and trade press. |
| `migrations/` | SQL migrations. The API applies them on startup when `RUN_MIGRATIONS=true`. |
| `packages/api-client` | TypeScript client generated from the OpenAPI document. |
| `apps/web` | Next.js site. |

## Requirements

- Rust 1.98 (pinned in `rust-toolchain.toml`, so rustup installs it)
- Node 22+ and pnpm
- Docker

## Getting started

```sh
cp .env.example .env
pnpm install
pnpm db:up          # Postgres 17 + pgvector on localhost:5434
pnpm seed           # runs the migrations and imports data/
pnpm dev:api        # http://127.0.0.1:4000, docs at /docs
pnpm dev:web        # http://localhost:3000 (in a second terminal)
pnpm worker:once    # fetches every due feed once; `pnpm dev:worker` keeps running
```

## Adding data

Each organization is one file, `data/organizations/<slug>.yaml`. It holds the
organization, the products it makes and, under each product, the prices that
providers charge for it:

```yaml
slug: meta
name: Meta
kinds: [model_provider]          # model_provider, inference_provider, llm_gateway, gpu_cloud,
                                 # hyperscaler, hardware, open_source, investor, other
tagline: Publisher of the open-weights Llama models.
website: https://ai.meta.com
hq_country: US
products:
  - slug: llama-3-1-70b
    name: Llama 3.1 70B
    type: model                  # model, inference_api, gateway, gpu_instance, oss_project,
                                 # hardware, platform, other
    launched_at: 2024-07-23
    model: { parameters_b: 70, context_window: 128000, open_weights: true }
    offerings:
      - provider: groq           # slug of another organization file
        input: 0.59              # USD per million input tokens
        output: 0.79             # USD per million output tokens
        source_url: https://groq.com/pricing/
        observed_on: 2024-11-01
```

GPU offerings use `gpu_type`, `gpu_count` and `per_hour` instead of `input` and `output`.

Open-source products take `github: owner/name`; the worker syncs their stars, forks,
license and latest release every 6 hours, and keeps one star count per day for trends.

Funding rounds go in the organization that raised them. Investors are slugs of other
organization files; anything they cannot express goes in `notes`. Amounts are stored as
announced, in `currency` (default USD), and are never converted:

```yaml
funding:
  - round: series_c               # lower_snake_case: seed, series_a, strategic, debt, ...
    announced_on: 2025-09-09
    amount: 1700000000
    valuation: 11700000000        # post-money
    currency: EUR
    lead: [asml]
    investors: [a16z, nvidia]
    source_url: https://mistral.ai/news/...
```

Only add a round with a `source_url` to the announcement.

`pnpm seed:check` validates the files without writing anything. `pnpm seed` imports them.
The import is idempotent: rows are upserted by slug, and an offering is added only when it
is a new observation. To record a price change, add a new offering with a newer `observed_on`
rather than editing the old one. The old one stays as price history. Unknown fields, broken
references and bad slugs are rejected before anything is written.

### The landscape list and the archive

`data/landscape/` holds the market list the directory was extended from: `platforms.tsv` and
`software.tsv` as given, `verification.json` (what was fetched for each row: HTTP status,
redirects, page titles and descriptions, GitHub repository facts) and `report.json` (every
row that was merged, skipped or created, and why). `scripts/landscape.py` turns them into
`data/organizations/*.yaml` and logos; it writes only fetched or listed facts, never
headquarters, founding years or funding. Logos come from the organization's own site icon or
its verified GitHub organization, and each source is in `apps/web/public/logos/sources.json`.

`pnpm archive:export` writes everything the worker fetched (market listings, held prices,
adapter state, news with its tags and triage, repository stats) to `data/archive/*.jsonl`,
keyed by slugs and URLs instead of row ids. `pnpm archive:import` loads it into a fresh
database and rebuilds the market summaries in about ten seconds, so a new environment needs
no refetch: `pnpm seed && pnpm archive:import`. The archive stays out of the Docker image.

To use a different Postgres port, set `POSTGRES_PORT` for Docker Compose and update
`DATABASE_URL` to match.

## News

The worker fetches each enabled feed every `interval_minutes` (default 60), sending the
stored `ETag` and `Last-Modified` so unchanged feeds cost one 304. A feed that fails backs
off: its interval doubles per consecutive failure, up to 24 hours. Items are deduplicated
by canonical URL (lower-case host without `www.`, no fragment, no tracking parameters).

Each item is tagged with the organizations it mentions by name. The rules live in the
organization file:

```yaml
feeds:                           # the organization's own blog; its items are tagged with it
  - slug: lambda-blog
    name: Lambda Blog
    url: https://lambda.ai/blog/rss.xml
tagging:
  aliases: [Lambda Labs]         # other names to match, as whole words
  exclude: [AWS Lambda]          # phrases that contain a name but mean something else
  match_case: true               # for names that are also common words
  match_name: false              # only the aliases match: for names like "Jan" or "Modal"
```

Items from an organization's own feed are published at once (`auto_approve` defaults to
true). Items from `data/sources.yaml` wait for review. After changing tagging rules, run
`pnpm seed` and then `pnpm worker:retag` to apply them to stored items.

When a feed will not parse, save it and run
`cargo run -p ih-ingest --example parse_file -- feed.xml https://its.url/` to see why.

## Market adapters

Prices come from the providers themselves, through adapters in
`crates/ingest/src/adapters/`. Each returns listings in common units: USD per million
tokens for chat models, USD per hour (with a GPU count) for GPU configurations, and for
other models their own unit: per million input tokens (embeddings), per image, per video
second, per character (text to speech) and per audio second (transcription). These are
compared at `/market/media`.

Every adapter reads its whole catalog. List endpoints go through `adapters/paging.rs`,
which follows offset/limit, page numbers, next links and cursors, stops at a repeated
page or a loop, and caps a walk at 200 pages. Each catalog spec records how its endpoint
pages; most answer with the full list at once, OpenRouter, Linode and Vultr page.

| Kind | Adapters |
|---|---|
| Model APIs and routers | OpenRouter, Liquid Inference (order book), Hugging Face Inference Providers (one price per hosting provider), DeepInfra, Novita, Chutes, SambaNova, Atlas Cloud, Inference.net, Featherless, Venice, Vercel AI Gateway, Requesty |
| GPU clouds and marketplaces | RunPod, Hyperbolic, CoreWeave, Lambda, Vultr, Akamai (Linode), Verda (DataCrunch), Cudo, Vast.ai and Akash (medians of host offers), io.net, Shadeform (18 resold clouds), Oracle Cloud, Microsoft Azure |

Each adapter runs on its own interval: hourly for marketplaces and order books, daily
for price lists. Every run stores everything it saw in `market_listings`.

The site never calls providers and never aggregates on request. The data flows one way:

```
providers ──(worker: run_adapter, every page)──▶ market_listings
market_listings ──(worker: refresh_market)──▶ market_models · market_gpus · market_providers · market_stats
summary tables ──(API: indexed, paginated reads)──▶ web
```

After an adapter run the worker queues one `refresh_market` job 20 seconds out, so the
adapters of one scheduler pass share it; summaries older than an hour are rebuilt
regardless. A rebuild takes under a second for ~26,000 listings and swaps all tables
in one transaction. Index pages read the summaries and page with `limit`/`offset`
along `market_models_rank_idx`; detail pages read the listings of one model or GPU. Listings that a product maps to also become offerings of that
product, with full price history:

```yaml
products:
  - slug: nvidia-h100
    listings:                    # the provider's own id for the product
      runpod: [NVIDIA H100 80GB HBM3]
      hyperbolic: [h100 sxm5]
      coreweave: [NVIDIA HGX H100]
```

A changed price adds an offering row; an unchanged price moves the row's
`confirmed_at`. A listing that disappears ends its offering. Guards keep bad data out:

- a run that returns less than half the listings of the last one, or a page whose
  layout changed, fails and changes nothing;
- a price outside plausible bounds, or more than 5× from the stored one, is held for
  review at `/admin/market` while the old price stays live (order books and host
  marketplaces only get the bounds check);
- jobs that run out of attempts are kept in `jobs_failed`.

GPU names are normalized (`NVIDIA H100 80GB HBM3`, `h100 sxm5`, `H100_80G` are all
`H100 SXM`), and models are matched across providers by a normalized id
(`deepseek-ai/DeepSeek-V3.2` and `deepseek/deepseek-v3.2:free` are `deepseek-v3.2`).

`cargo run -p ih-ingest --example live_parse [adapter ...]` fetches adapters live and
prints what they would store, without a database. To add a provider, see
[CONTRIBUTING.md](CONTRIBUTING.md).

## News triage with an LLM

With `LIQUID_BASE_URL` and `LIQUID_API_KEY` set, the worker sends pending news to an
OpenAI-compatible exchange (Liquid Inference, `liquid.auto`) for a topic, a relevance
grade, a suggested verdict with confidence, a one-sentence summary, and the
organizations the item is about, chosen from a closed list of directory slugs. Items
are published automatically only on a confident approve for core news from an
organization's own feed; everything else waits for an editor, who sees the
suggestion. Spend is capped per call, per pass and per day (`MAX_ENRICH_USD_PER_DAY`).
`pnpm worker:enrich` triages the backlog once.

## Admin

Set `ADMIN_TOKEN` (32+ characters, `openssl rand -hex 32`) for both the API and the web
server. That enables the admin API under `/v1/admin` (bearer token) and the review pages
at `/admin`, where editors approve or reject queued items, correct their organizations,
review held prices, and see which feeds and adapters are failing. Without `ADMIN_TOKEN`
both answer 404. `ADMIN_ALLOWED_IPS` (addresses or CIDR networks) additionally limits the
admin API to those clients; behind a proxy, set `CLIENT_IP_HEADER` to the header it sets,
such as `X-Real-IP` on Railway.

## Deploy

The backend runs on Railway from the repository's `Dockerfile` (one image for the API,
the worker and the seed), with Postgres and pgvector on a volume; the web app runs on
Vercel. The Railway project is described in `.railway/railway.ts`. See
[DEPLOY.md](DEPLOY.md) for the first deploy, secrets and operations, and
`docker-compose.prod.yml` to run the same stack locally.

The worker serves `/healthz`, `/readyz` and Prometheus `/metrics` on
`WORKER_METRICS_PORT` (default 9090), and every service logs JSON with
`LOG_FORMAT=json`.

## Changing the API

The Rust types are the source of truth. After changing a route or a type in `crates/core`, run:

```sh
pnpm gen:api        # writes packages/api-client/openapi.json and src/schema.d.ts
```

Commit both generated files. CI fails if they are stale.

## Checks

```sh
pnpm lint           # cargo fmt --check, clippy -D warnings, eslint
pnpm typecheck      # web, API client and .railway/railway.ts
pnpm test           # needs Postgres running: each DB test creates and drops its own database
```

## Roadmap

1. **Foundation** (done): workspace, schema, read API, directory pages.
2. **Directory MVP** (done): YAML seed importer, product and model pages with pricing,
   SEO (sitemap, robots, JSON-LD, canonical URLs).
3. **News pipeline** (done): Postgres job queue, feed ingestion with conditional requests,
   name-based tagging, admin review queue.
4. **Comparisons** (done): model and GPU price tables, price history, GitHub stats,
   funding rounds and investor portfolios.
5. **Market coverage** (done): 27 price adapters, market-wide indexes, price guards,
   LLM news triage, container deploy.
6. **Growth:** price history charts per market item, semantic search, newsletter,
   public API, submissions.
