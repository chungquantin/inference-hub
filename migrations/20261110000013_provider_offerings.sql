-- One list of what each provider sells, whatever the source.
--
-- A provider's page used to show three lists: the market catalog from its
-- adapter, the curated offerings ("Tracked prices") and the curated products
-- it serves ("Sold by"). They overlap and disagree. The worker now merges
-- them into one row per provider and item, with the provider's own price,
-- the market's, and a link to the curated product page when one exists.

CREATE TABLE market_provider_offerings (
    organization_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    -- `model`, `gpu`, `embedding`, `image`, `video`, `speech`, `transcription`.
    kind            text NOT NULL,
    -- The market key: a model key, or a GPU type.
    item_key        text NOT NULL,
    name            text NOT NULL,
    maker           text,
    -- The curated product, when editors mapped this item.
    product_id      uuid REFERENCES products (id) ON DELETE SET NULL,
    -- `adapter` (collected live), `curated` (from the data files) or both.
    sources         text[] NOT NULL,
    -- Listings of this item at this provider (variants, regions, configurations).
    variants        integer NOT NULL,
    -- This provider's lowest price, in the kind's unit: blended USD per 1M
    -- tokens (chat), USD per GPU-hour (GPUs), USD per unit (others).
    price           double precision,
    input_per_mtok  double precision,
    output_per_mtok double precision,
    unit            text,
    -- The whole market, for comparison.
    market_providers integer,
    market_min      double precision,
    market_median   double precision,
    -- 1 = cheapest in the market (ties share the rank).
    rank            integer,
    context_window  integer,
    source_url      text,
    last_seen_at    timestamptz,
    PRIMARY KEY (organization_id, kind, item_key)
);
CREATE INDEX market_provider_offerings_list_idx ON market_provider_offerings
    (organization_id, kind, market_providers DESC NULLS LAST, price, item_key);
