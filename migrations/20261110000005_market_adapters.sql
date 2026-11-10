-- Market adapters: automated price collection from gateways (OpenRouter,
-- Liquid Inference) and neoclouds (RunPod, Hyperbolic, CoreWeave).
--
-- An adapter run stores every listing it sees in `market_listings`. Listings
-- that an editor mapped to a product (`product_listing_ids`) also become
-- offerings, so they show up in comparisons and price history.

-- Offerings: who recorded them, when they were last confirmed, when they ended.
ALTER TABLE offerings
    -- `manual` for the curated YAML, or the id of the adapter that saw it.
    ADD COLUMN source       text NOT NULL DEFAULT 'manual',
    -- Last time the same price was seen again. A new price is a new row.
    ADD COLUMN confirmed_at timestamptz,
    -- The provider stopped offering it. An ended latest row hides the offering.
    ADD COLUMN ended_at     timestamptz;

DROP VIEW latest_offerings;
-- The newest observation of each (product, provider, configuration), unless
-- that observation has ended. Older rows stay in `offerings` as price history.
CREATE VIEW latest_offerings AS
SELECT * FROM (
    SELECT DISTINCT ON (product_id, provider_id, coalesce(gpu_type, ''),
                        coalesce(gpu_count, 0), coalesce(region, ''))
           *
    FROM offerings
    ORDER BY product_id, provider_id, coalesce(gpu_type, ''), coalesce(gpu_count, 0),
             coalesce(region, ''), observed_at DESC, id
) latest
WHERE ended_at IS NULL;

-- Health of each adapter, like `sources` for feeds.
CREATE TABLE adapter_state (
    adapter              text PRIMARY KEY,
    last_run_at          timestamptz,
    last_success_at      timestamptz,
    last_error           text,
    consecutive_failures integer NOT NULL DEFAULT 0,
    -- Counts of the last successful run.
    listings_seen        integer NOT NULL DEFAULT 0,
    offerings_added      integer NOT NULL DEFAULT 0
);

-- Everything an adapter saw, one row per adapter and configuration.
CREATE TABLE market_listings (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    adapter         text NOT NULL,
    -- Unique per configuration within the adapter, such as `h200-sxm5|8|us-east-2`.
    listing_key     text NOT NULL,
    -- The provider's id of the model or GPU, which editors map to a product.
    external_id     text NOT NULL,
    kind            text NOT NULL CHECK (kind IN ('model', 'gpu')),
    name            text NOT NULL,
    -- Token-priced (models)
    maker           text,
    context_window  integer,
    price_input_per_mtok  numeric(14, 6),
    price_output_per_mtok numeric(14, 6),
    -- Time-priced (GPUs)
    gpu_type        text,
    gpu_count       smallint,
    price_per_hour  numeric(12, 4),
    region          text,
    currency        char(3) NOT NULL DEFAULT 'USD',
    source_url      text,
    -- Provider-specific details kept for reference, such as spot prices.
    extra           jsonb NOT NULL DEFAULT '{}'::jsonb,
    first_seen_at   timestamptz NOT NULL DEFAULT now(),
    last_seen_at    timestamptz NOT NULL DEFAULT now(),
    delisted_at     timestamptz,
    UNIQUE (adapter, listing_key)
);
CREATE INDEX market_listings_external_idx ON market_listings (adapter, external_id);

-- Editor-made links from an adapter's listing to a product of the directory.
CREATE TABLE product_listing_ids (
    product_id  uuid NOT NULL REFERENCES products (id) ON DELETE CASCADE,
    adapter     text NOT NULL,
    external_id text NOT NULL,
    PRIMARY KEY (adapter, external_id)
);
CREATE INDEX product_listing_ids_product_idx ON product_listing_ids (product_id);
