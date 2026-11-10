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
| `crates/worker` | Ingestion jobs (RSS, GitHub, pricing). It's a stub for now. |
| `crates/seed` | Validates and imports the curated YAML in `data/`. |
| `data/organizations/` | Curated data: one YAML file per organization, with its products and prices. |
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

`pnpm seed:check` validates the files without writing anything. `pnpm seed` imports them.
The import is idempotent: rows are upserted by slug, and an offering is added only when it
is a new observation. To record a price change, add a new offering with a newer `observed_on`
rather than editing the old one. The old one stays as price history. Unknown fields, broken
references and bad slugs are rejected before anything is written.

To use a different Postgres port, set `POSTGRES_PORT` for Docker Compose and update
`DATABASE_URL` to match.

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
3. **News pipeline:** worker job queue, RSS ingestion, LLM tagging, admin review queue.
4. **Comparisons:** pricing offerings, model and GPU price tables, GitHub stats, funding.
5. **Growth:** semantic search, newsletter, public API, submissions.
