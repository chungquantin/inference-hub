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
pnpm dev:api        # http://127.0.0.1:4000, docs at /docs
pnpm dev:web        # http://localhost:3000 (in a second terminal)
```

The API runs the migrations on startup. The second migration seeds the taxonomy and a
few organizations, so the site has data to show from the start.

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
pnpm test
```

## Roadmap

1. **Foundation** (done): workspace, schema, read API, directory pages.
2. **Directory MVP:** YAML seed importer, product and model pages, SEO (sitemap, JSON-LD).
3. **News pipeline:** worker job queue, RSS ingestion, LLM tagging, admin review queue.
4. **Comparisons:** pricing offerings, model and GPU price tables, GitHub stats, funding.
5. **Growth:** semantic search, newsletter, public API, submissions.
