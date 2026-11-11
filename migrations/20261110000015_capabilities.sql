-- What models can do: input and output modalities (`in:image`, `out:audio`)
-- and features (`tools`, `json`, `reasoning`, `web`, `caching`), as each
-- provider reports them. Summaries hold the union across providers.
ALTER TABLE market_listings ADD COLUMN capabilities text[] NOT NULL DEFAULT '{}';
ALTER TABLE market_models ADD COLUMN capabilities text[] NOT NULL DEFAULT '{}';
ALTER TABLE market_provider_offerings ADD COLUMN capabilities text[] NOT NULL DEFAULT '{}';
-- Filtering the index by capability (`capabilities @> '{tools}'`).
CREATE INDEX market_models_capabilities_idx ON market_models USING gin (capabilities);
