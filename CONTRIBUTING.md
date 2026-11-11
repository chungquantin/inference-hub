# Contributing

Inference Hub aims to cover the whole inference market: every model API,
router and GPU cloud, small to large. The most useful contributions are new
providers (an adapter, or curated data) and corrections.

## Ground rules for data

- **Only sourced facts.** Every price, funding round and date needs a public,
  official `source_url` (the provider's pricing page, API or announcement).
  Never fill a value from memory or a third-party summary.
- **Leave out what you cannot verify.** An organization file can have just a
  name, kinds and website; add `hq_country` or `founded_year` only with a
  source.
- **History is append-only.** To record a price change, add a newer offering
  rather than editing the old one.

## Set up

See the README's *Getting started*. In short: `cp .env.example .env`,
`pnpm install`, `pnpm db:up`, `pnpm seed`.

## Adding a provider

### 1. Find its public price source

Prefer, in order: a keyless JSON API, a public price list file, a pricing page.
Check it from a shell first, and note how prices are expressed (per token, per
million tokens, cents, per GPU or per instance).

Before scraping a page, check its `robots.txt` and terms. Never use an
authenticated API with personal credentials.

### 2. Write the adapter

Adapters live in `crates/ingest/src/adapters/`. Pick the shortest path:

| The source is | Do this |
|---|---|
| An OpenAI-style model list with prices (`{"data": [{ "id", "pricing": {...} }]}`) | Add a `CatalogSpec` to `CATALOGS` in `catalog.rs`: an entry function that reads one model. |
| One JSON document of GPU configurations | A new module with `simple_gpu_adapter!` and a `parse` function (see `vultr.rs`, `verda.rs`). |
| Anything else | Implement `GatewayAdapter` or `NeoCloudAdapter` directly (see `runpod.rs`, `lambda.rs`). |

Rules every adapter follows:

- **Convert to the common units.** Models: USD per million input and output
  tokens. GPUs: USD per hour for the whole configuration, with `gpu_count`.
- **Normalize GPU names** with `gpu::normalize`, and add the cloud's spellings
  to its test. An unknown GPU is skipped, never guessed.
- **Fail rather than store junk.** Return `AdapterError::Format` when the
  shape is unexpected or the result is empty. The store also refuses runs that
  shrink by half, and holds implausible prices for review.
- **Read the whole catalog.** Find out how the endpoint paginates (try
  `?limit=`, `?offset=`, `?page=`, and look for `next`, `cursor` or a total in
  the body) and fetch through `paging::fetch_all` with the matching `Paging`.
  Ask for every kind of model the endpoint can list (OpenRouter needs
  `output_modalities=all`, Venice `type=all`).
- **Price non-chat models in their unit** with `ModelListing::media`: per image,
  per video second, per character, per audio second, or per million input
  tokens for embeddings. Skip prices that only convert with assumptions (per
  GPU second, per image token).
- **Pick an interval.** `HOURLY` for marketplaces and order books, `DAILY` for
  price lists. Mark order books and host marketplaces `volatile`.
- **Write unit tests** for the parser from a trimmed real response, including
  the entries it must skip.

Register the adapter in `market_adapters()` in `adapters/mod.rs` and add its id
to `ADAPTERS` in `crates/seed/src/data.rs`. Both are checked against each
other in the tests.

### 3. Add the organization

Create `data/organizations/<slug>.yaml` with at least `slug`, `name`, `kinds`
and `website`, matching the adapter's `organization_slug`. Add tagging aliases
if the company goes by other names.

### 4. Check it against the live source

```sh
cargo run -p ih-ingest --example live_parse <adapter-id>
```

prints what the adapter would store, without a database. Then run it for real:

```sh
pnpm seed && pnpm worker:once
```

and look at `/market`, `/market/gpus` or `/market/models`, and `/admin/market`.

## Changing the API

Types in `crates/core` are the API contract. After changing a route or a type,
run `pnpm gen:api` and commit `packages/api-client/openapi.json` and
`src/schema.d.ts`.

## Migrations

Add a new file in `migrations/` named `YYYYMMDDHHMMSS_what.sql`; never edit one
that has shipped. Keep migrations additive so the previous release keeps
working during a deploy.

## Before you open a pull request

```sh
pnpm lint             # cargo fmt --check, clippy -D warnings, eslint
pnpm typecheck        # web, API client and the Railway file
pnpm test             # needs Postgres from `pnpm db:up`
pnpm seed:check
```

CI runs the same checks and builds the Docker image.

Commits follow [Conventional Commits](https://www.conventionalcommits.org/):
`feat(ingest): add the X adapter`, `fix(web): ...`, `docs: ...`.
