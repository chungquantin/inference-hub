-- Comparisons: GitHub stats for open-source products, and funding rounds that
-- the curated data can upsert.

-- GitHub repositories ---------------------------------------------------------

ALTER TABLE products
    -- "owner/name" of the product's GitHub repository.
    ADD COLUMN github_repo text
        CHECK (github_repo ~ '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$');
CREATE UNIQUE INDEX products_github_repo_idx ON products (lower(github_repo));

-- The latest state of each repository, overwritten on every sync. A row with
-- null counts records a repository whose first sync failed, so it waits for the
-- next interval instead of being retried every minute.
CREATE TABLE repo_stats (
    product_id     uuid PRIMARY KEY REFERENCES products (id) ON DELETE CASCADE,
    stars          integer,
    forks          integer,
    open_issues    integer,
    license        text,
    archived       boolean NOT NULL DEFAULT false,
    pushed_at      timestamptz,
    latest_release text,
    released_at    timestamptz,
    -- HTTP validators of the repository request.
    etag           text,
    synced_at      timestamptz NOT NULL DEFAULT now(),
    last_error     text
);

-- One row per repository per day, for star history.
CREATE TABLE repo_star_history (
    product_id uuid NOT NULL REFERENCES products (id) ON DELETE CASCADE,
    day        date NOT NULL,
    stars      integer NOT NULL,
    PRIMARY KEY (product_id, day)
);

-- Funding ----------------------------------------------------------------------

-- Rounds come from the curated data, keyed by organization, round name and date,
-- so a re-import updates a round instead of adding it again.
ALTER TABLE funding_rounds
    -- Amounts are stored as announced, in this currency. Renamed columns below.
    ADD COLUMN currency char(3) NOT NULL DEFAULT 'USD',
    -- Context the investor list cannot hold, such as investors not in the directory.
    ADD COLUMN notes text,
    ADD CONSTRAINT funding_rounds_natural_key UNIQUE (organization_id, round, announced_on),
    ADD CONSTRAINT funding_rounds_amount_positive CHECK (amount_usd IS NULL OR amount_usd > 0),
    ADD CONSTRAINT funding_rounds_valuation_positive CHECK (valuation_usd IS NULL OR valuation_usd > 0);
ALTER TABLE funding_rounds RENAME COLUMN amount_usd TO amount;
ALTER TABLE funding_rounds RENAME COLUMN valuation_usd TO valuation;

-- Comparisons ------------------------------------------------------------------

-- Latest-offering lookups scan per provider and configuration.
CREATE INDEX offerings_latest_idx ON offerings
    (product_id, provider_id, gpu_type, gpu_count, region, observed_at DESC);
CREATE INDEX offerings_gpu_type_idx ON offerings (gpu_type) WHERE gpu_type IS NOT NULL;

-- The newest observation of each (product, provider, configuration). Older rows
-- stay in `offerings` as price history.
CREATE VIEW latest_offerings AS
SELECT DISTINCT ON (product_id, provider_id, coalesce(gpu_type, ''), coalesce(gpu_count, 0),
                    coalesce(region, ''))
       *
FROM offerings
ORDER BY product_id, provider_id, coalesce(gpu_type, ''), coalesce(gpu_count, 0),
         coalesce(region, ''), observed_at DESC, id;
