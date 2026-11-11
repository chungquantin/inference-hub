-- Global search: trigram indexes so `ILIKE '%text%'` and similarity ranking
-- stay fast over every searchable table.
CREATE INDEX IF NOT EXISTS products_name_trgm_idx ON products USING gin (name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS products_slug_trgm_idx ON products USING gin (slug gin_trgm_ops);
CREATE INDEX IF NOT EXISTS organizations_slug_trgm_idx ON organizations USING gin (slug gin_trgm_ops);
CREATE INDEX IF NOT EXISTS market_models_key_trgm_idx ON market_models USING gin (model_key gin_trgm_ops);
CREATE INDEX IF NOT EXISTS market_models_name_trgm_idx ON market_models USING gin (name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS market_gpus_type_trgm_idx ON market_gpus USING gin (gpu_type gin_trgm_ops);
CREATE INDEX IF NOT EXISTS news_items_title_trgm_idx ON news_items USING gin (title gin_trgm_ops);
CREATE INDEX IF NOT EXISTS market_provider_offerings_name_trgm_idx
    ON market_provider_offerings USING gin (name gin_trgm_ops);
-- Maker filter and sorting of the model index.
CREATE INDEX IF NOT EXISTS market_models_maker_idx ON market_models (kind, maker);
