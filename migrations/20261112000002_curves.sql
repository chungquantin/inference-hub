-- Price curves.
--
-- `price_snapshots` keeps one row per day for every GPU type and chat model: the
-- cheapest, median and priciest price that day. The market summaries only hold the
-- present, so history starts the day this table does and grows from the refresh job.
--
-- `gpu_term_prices` holds the term structure: what one GPU-hour costs when bought
-- as spot, on demand, or committed for a term, by provider. Only providers that
-- publish those prices appear; a missing tier is missing, never interpolated.

CREATE TABLE price_snapshots (
    day       date NOT NULL,
    -- 'gpu' (USD per GPU-hour) or 'model' (USD per million tokens, 3:1 input:output blend).
    kind      text NOT NULL CHECK (kind IN ('gpu', 'model')),
    item_key  text NOT NULL,
    providers integer NOT NULL,
    min_price double precision NOT NULL,
    median_price double precision NOT NULL,
    max_price double precision,
    PRIMARY KEY (kind, item_key, day)
);

CREATE TABLE gpu_term_prices (
    organization_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    gpu_type        text NOT NULL,
    -- 'spot' (can be interrupted), 'on_demand', or 'committed' (reserved for term_months).
    tier            text NOT NULL CHECK (tier IN ('spot', 'on_demand', 'committed')),
    -- 0 for spot and on demand.
    term_months     integer NOT NULL DEFAULT 0,
    per_gpu_hour    double precision NOT NULL CHECK (per_gpu_hour > 0),
    source          text NOT NULL,
    source_url      text,
    observed_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, gpu_type, tier, term_months),
    CHECK ((tier = 'committed') = (term_months > 0))
);
CREATE INDEX gpu_term_prices_gpu_idx ON gpu_term_prices (gpu_type);
