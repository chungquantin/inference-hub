# Inference Hub

A directory and news site for the inference market: model providers, LLM gateways,
open-source projects, GPU clouds, hyperscalers, hardware makers and investors.

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
| `crates/seed` | Validates and imports the curated YAML in `data/`. |
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
```

Items from an organization's own feed are published at once (`auto_approve` defaults to
true). Items from `data/sources.yaml` wait for review. After changing tagging rules, run
`pnpm seed` and then `pnpm worker:retag` to apply them to stored items.

When a feed will not parse, save it and run
`cargo run -p ih-ingest --example parse_file -- feed.xml https://its.url/` to see why.

## Market adapters

Prices also come from the providers themselves, through adapters in
`crates/ingest/src/adapters/`. There are three kinds of adapter, each a trait:

| Trait | Returns | Adapters |
|---|---|---|
| `GatewayAdapter` | model API prices (`ModelListing`) | `OpenRouterAdapter` (public model list), `LiquidInferenceAdapter` (best live offer in the exchange's order book) |
| `NeoCloudAdapter` | GPU instance prices (`GpuListing`) | `RunPodAdapter` (GraphQL, Secure Cloud price), `HyperbolicAdapter` (rental options), `CoreWeaveAdapter` (reads the pricing page) |
| `NewsAdapter` | articles (`FeedEntry`) | `RssNewsAdapter` (RSS, Atom, JSON Feed) |

The worker runs every market adapter every 6 hours. Each run stores everything it saw in
`market_listings`. Listings that a product maps to become offerings at the adapter's
organization:

```yaml
products:
  - slug: nvidia-h100
    listings:                    # the provider's own id for the product
      runpod: [NVIDIA H100 80GB HBM3]
      hyperbolic: [h100 sxm5]
      coreweave: [NVIDIA HGX H100]
```

A changed price adds an offering row, so price history stays complete; an unchanged price
moves the row's `confirmed_at`, which the site shows as the date checked. A listing that
disappears ends its offering, which takes it out of comparisons; curated offerings are
never ended by an adapter. A run that returns less than half the listings of the last one,
or a page whose layout changed, fails and changes nothing. `/admin/market` shows each
adapter's health and the listings no product maps to yet.

GPU names are normalized (`NVIDIA H100 80GB HBM3`, `h100 sxm5` and `NVIDIA HGX H100` are all
`H100 SXM`), so the GPU comparison lines them up. To add a provider, implement one of the
traits, give it an id and the slug of its organization, and register it in
`market_adapters()`.

## Admin

Set `ADMIN_TOKEN` (32+ characters, `openssl rand -hex 32`) for both the API and the web
server. That enables the admin API under `/v1/admin` (bearer token) and the review pages
at `/admin`, where editors approve or reject queued items, correct their organizations,
and see which feeds are failing. Without `ADMIN_TOKEN` both answer 404.

## Changing the API

The Rust types are the source of truth. After changing a route or a type in `crates/core`, run:

```sh
pnpm gen:api        # writes packages/api-client/openapi.json and src/schema.d.ts
```

Commit both generated files. CI fails if they are stale.

## Checks

```sh
pnpm lint           # cargo fmt --check, clippy -D warnings, eslint
pnpm typecheck
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
5. **Growth:** semantic search, newsletter, public API, submissions.
