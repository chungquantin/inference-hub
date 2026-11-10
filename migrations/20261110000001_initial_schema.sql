-- Initial schema for Inference Hub.
-- Organizations are the hub; products, offerings, funding and news hang off them.

CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS vector;

-- updated_at maintenance ----------------------------------------------------

CREATE FUNCTION set_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Enums ---------------------------------------------------------------------

-- An organization can play several roles (e.g. a hyperscaler that also trains models and invests).
CREATE TYPE org_kind AS ENUM (
    'model_provider',
    'inference_provider',
    'llm_gateway',
    'gpu_cloud',
    'hyperscaler',
    'hardware',
    'open_source',
    'investor',
    'other'
);

CREATE TYPE org_status AS ENUM ('active', 'acquired', 'defunct');

CREATE TYPE product_type AS ENUM (
    'model',
    'inference_api',
    'gateway',
    'gpu_instance',
    'oss_project',
    'hardware',
    'platform',
    'other'
);

CREATE TYPE news_status AS ENUM ('pending', 'approved', 'rejected');

-- Taxonomy ------------------------------------------------------------------

CREATE TABLE categories (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        text NOT NULL UNIQUE,
    name        text NOT NULL,
    description text,
    parent_id   uuid REFERENCES categories (id) ON DELETE SET NULL,
    sort_order  integer NOT NULL DEFAULT 0,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX categories_parent_idx ON categories (parent_id);
CREATE TRIGGER categories_updated_at BEFORE UPDATE ON categories
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Organizations -------------------------------------------------------------

CREATE TABLE organizations (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        text NOT NULL UNIQUE,
    name        text NOT NULL,
    kinds       org_kind[] NOT NULL DEFAULT '{}',
    tagline     text,
    description text,
    website     text,
    logo_url    text,
    hq_country  char(2),             -- ISO 3166-1 alpha-2
    founded_year smallint,
    status      org_status NOT NULL DEFAULT 'active',
    links       jsonb NOT NULL DEFAULT '{}'::jsonb, -- {"github": "...", "x": "...", "docs": "..."}
    search      tsvector GENERATED ALWAYS AS (
        setweight(to_tsvector('english', coalesce(name, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(tagline, '')), 'B') ||
        setweight(to_tsvector('english', coalesce(description, '')), 'C')
    ) STORED,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX organizations_kinds_idx ON organizations USING gin (kinds);
CREATE INDEX organizations_search_idx ON organizations USING gin (search);
CREATE INDEX organizations_name_trgm_idx ON organizations USING gin (name gin_trgm_ops);
CREATE TRIGGER organizations_updated_at BEFORE UPDATE ON organizations
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE organization_categories (
    organization_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    category_id     uuid NOT NULL REFERENCES categories (id) ON DELETE CASCADE,
    PRIMARY KEY (organization_id, category_id)
);
CREATE INDEX organization_categories_category_idx ON organization_categories (category_id);

-- Products ------------------------------------------------------------------

CREATE TABLE products (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    slug            text NOT NULL UNIQUE,
    name            text NOT NULL,
    product_type    product_type NOT NULL,
    tagline         text,
    description     text,
    url             text,
    launched_at     date,
    metadata        jsonb NOT NULL DEFAULT '{}'::jsonb, -- type-specific attributes
    search          tsvector GENERATED ALWAYS AS (
        setweight(to_tsvector('english', coalesce(name, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(tagline, '')), 'B') ||
        setweight(to_tsvector('english', coalesce(description, '')), 'C')
    ) STORED,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX products_organization_idx ON products (organization_id);
CREATE INDEX products_type_idx ON products (product_type);
CREATE INDEX products_search_idx ON products USING gin (search);
CREATE TRIGGER products_updated_at BEFORE UPDATE ON products
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Model-specific attributes (1:1 with products where product_type = 'model').
CREATE TABLE models (
    product_id      uuid PRIMARY KEY REFERENCES products (id) ON DELETE CASCADE,
    family          text,
    parameters_b    numeric(10, 2),  -- billions of parameters, total
    active_params_b numeric(10, 2),  -- for MoE models
    context_window  integer,
    modalities      text[] NOT NULL DEFAULT '{}', -- text, image, audio, video
    license         text,
    open_weights    boolean NOT NULL DEFAULT false
);

-- Who sells what, at what price. Append-only: each row is an observation,
-- which gives price history for free.
CREATE TABLE offerings (
    id                     uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id             uuid NOT NULL REFERENCES products (id) ON DELETE CASCADE,
    provider_id            uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    -- token-priced offerings (model APIs)
    price_input_per_mtok   numeric(12, 6),
    price_output_per_mtok  numeric(12, 6),
    -- time-priced offerings (GPU instances)
    gpu_type               text,
    gpu_count              smallint,
    price_per_hour         numeric(12, 4),
    region                 text,
    currency               char(3) NOT NULL DEFAULT 'USD',
    source_url             text,
    observed_at            timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX offerings_product_observed_idx ON offerings (product_id, observed_at DESC);
CREATE INDEX offerings_provider_idx ON offerings (provider_id);

-- Funding -------------------------------------------------------------------

CREATE TABLE funding_rounds (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    round           text NOT NULL,       -- seed, series_a, ...
    amount_usd      bigint,
    valuation_usd   bigint,
    announced_on    date NOT NULL,
    source_url      text,
    created_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX funding_rounds_org_idx ON funding_rounds (organization_id, announced_on DESC);

CREATE TABLE funding_round_investors (
    funding_round_id uuid NOT NULL REFERENCES funding_rounds (id) ON DELETE CASCADE,
    investor_id      uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    is_lead          boolean NOT NULL DEFAULT false,
    PRIMARY KEY (funding_round_id, investor_id)
);
CREATE INDEX funding_round_investors_investor_idx ON funding_round_investors (investor_id);

-- Ingestion sources & news --------------------------------------------------

CREATE TABLE sources (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name        text NOT NULL,
    source_type text NOT NULL,            -- rss, github, scraper, manual
    config      jsonb NOT NULL DEFAULT '{}'::jsonb,
    enabled     boolean NOT NULL DEFAULT true,
    last_run_at timestamptz,
    last_error  text,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);
CREATE TRIGGER sources_updated_at BEFORE UPDATE ON sources
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE news_items (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    source_id    uuid REFERENCES sources (id) ON DELETE SET NULL,
    url          text NOT NULL UNIQUE,
    title        text NOT NULL,
    publisher    text,
    summary      text,
    image_url    text,
    published_at timestamptz NOT NULL,
    status       news_status NOT NULL DEFAULT 'pending',
    embedding    vector(1536),             -- for dedup / semantic search
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX news_items_published_idx ON news_items (status, published_at DESC);
CREATE TRIGGER news_items_updated_at BEFORE UPDATE ON news_items
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE news_item_organizations (
    news_item_id    uuid NOT NULL REFERENCES news_items (id) ON DELETE CASCADE,
    organization_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    PRIMARY KEY (news_item_id, organization_id)
);
CREATE INDEX news_item_organizations_org_idx ON news_item_organizations (organization_id);
