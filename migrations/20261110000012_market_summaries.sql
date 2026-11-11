-- Precomputed market summaries.
--
-- Adapters write raw listings (`market_listings`). Grouping ~26,000 listings
-- by model on every page view took seconds, so the worker computes the
-- summaries once after adapter runs (`refresh_market` job) and the API only
-- reads these tables, paging along their indexes.
--
-- Organizations are stored by id and joined at read time, so renames and
-- logos show at once without a refresh.

-- One row per model and kind: chat models (`model`) and models priced per
-- unit (`embedding`, `image`, `video`, `speech`, `transcription`).
CREATE TABLE market_models (
    kind            text NOT NULL,
    model_key       text NOT NULL,
    name            text NOT NULL,
    maker           text,
    -- Distinct sellers (a router's hosting provider counts as a seller).
    providers       integer NOT NULL,
    listings        integer NOT NULL,
    -- Organizations whose adapters list it, for the provider filter.
    organization_ids uuid[] NOT NULL,
    cheapest_organization_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    -- Chat models, USD per million tokens; blend is 3 input : 1 output.
    min_input_per_mtok    double precision,
    min_output_per_mtok   double precision,
    min_blended_per_mtok  double precision,
    median_blended_per_mtok double precision,
    max_context_window integer,
    -- Unit-priced models.
    unit            text,
    min_price       double precision,
    median_price    double precision,
    PRIMARY KEY (kind, model_key)
);
-- The index order: the most widely sold first, then the cheapest.
CREATE INDEX market_models_rank_idx ON market_models
    (kind, providers DESC, min_blended_per_mtok, min_price, model_key);
CREATE INDEX market_models_orgs_idx ON market_models USING gin (organization_ids);

CREATE TABLE market_gpus (
    gpu_type        text PRIMARY KEY,
    providers       integer NOT NULL,
    listings        integer NOT NULL,
    organization_ids uuid[] NOT NULL,
    cheapest_organization_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    min_per_gpu_hour    double precision NOT NULL,
    median_per_gpu_hour double precision NOT NULL,
    max_per_gpu_hour    double precision NOT NULL
);
CREATE INDEX market_gpus_rank_idx ON market_gpus (providers DESC, listings DESC, gpu_type);

CREATE TABLE market_providers (
    organization_id uuid PRIMARY KEY REFERENCES organizations (id) ON DELETE CASCADE,
    models          integer NOT NULL,
    model_listings  integer NOT NULL,
    media_models    integer NOT NULL,
    gpu_configurations integer NOT NULL,
    cheapest_count  integer NOT NULL,
    last_seen_at    timestamptz
);

-- One row: totals of the last refresh.
CREATE TABLE market_stats (
    id              boolean PRIMARY KEY DEFAULT true CHECK (id),
    providers       integer NOT NULL,
    model_providers integer NOT NULL,
    gpu_providers   integer NOT NULL,
    model_listings  integer NOT NULL,
    distinct_models integer NOT NULL,
    multi_provider_models integer NOT NULL,
    gpu_listings    integer NOT NULL,
    gpu_types       integer NOT NULL,
    media_listings  integer NOT NULL,
    media_models    integer NOT NULL,
    -- Freshest listing, and when the summaries were computed.
    updated_at      timestamptz,
    refreshed_at    timestamptz NOT NULL,
    refresh_ms      integer NOT NULL
);
