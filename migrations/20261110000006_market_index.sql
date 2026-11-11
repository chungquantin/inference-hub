-- Market-wide views over everything the adapters see, mapped or not.

-- The organization each adapter reports for, so market views can name and
-- link the provider without a code lookup.
ALTER TABLE adapter_state ADD COLUMN organization_id uuid REFERENCES organizations (id);

-- One key for a model across providers: providers write the same model as
-- `deepseek-ai/DeepSeek-V3.2`, `deepseek/deepseek-v3.2` or `DeepSeek-V3.2`.
-- Drops the maker prefix, OpenRouter's `:free`-style variants and deployment
-- suffixes (quantization, TEE), then lower-cases and joins words with `-`.
CREATE FUNCTION market_model_key(external_id text) RETURNS text
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
    SELECT trim(both '-' FROM regexp_replace(
        regexp_replace(
            regexp_replace(lower(regexp_replace(external_id, '^.*/', '')), ':.*$', ''),
            '[-_](fp8|fp4|bf16|int4|int8|awq|gptq|tee|turbo|instruct-fp8)$', ''),
        '[^a-z0-9.]+', '-', 'g'))
$$;

CREATE INDEX market_listings_model_key_idx ON market_listings (market_model_key(external_id))
    WHERE kind = 'model' AND delisted_at IS NULL;
CREATE INDEX market_listings_gpu_idx ON market_listings (gpu_type)
    WHERE kind = 'gpu' AND delisted_at IS NULL;
