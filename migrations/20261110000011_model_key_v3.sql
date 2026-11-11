-- A better key for matching one model across providers.
--
-- Providers spell one model many ways:
--   anthropic/claude-sonnet-4.5   claude-sonnet-4-5   bedrock/claude-sonnet-4-5@us-east-1
--   azure/gpt-6.1-sol@westeurope  claude-sonnet-4-5-eu-west-1   google/gemini-2.5-pro-batch
-- The key drops, in order:
--   1. everything up to the last `/` (maker or routing prefixes such as `vertex/`);
--   2. `@region` and `:variant` suffixes (`:free`, `:thinking` stay distinct below);
--   3. cloud region suffixes such as `-us-east-1`, `-eu-west-3`, `-europe-west1`;
--   4. deployment suffixes: quantization, confidential compute, `-batch`;
-- and writes version separators the same way: `4.5` and `4-5` both become `4.5`
-- when they join two single digits (`claude-sonnet-4-5`, `gemini-2-5`), while
-- dates (`2025-09-29`, `0731`) and sizes (`235b-a22b`) are left alone.
--
-- `:thinking` and `-turbo` name different behavior or a different model, so
-- they are kept.
CREATE OR REPLACE FUNCTION market_model_key(external_id text) RETURNS text
LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE AS $$
DECLARE
    k text := lower(external_id);
BEGIN
    k := regexp_replace(k, '^.*/', '');
    k := regexp_replace(k, '@.*$', '');
    -- Keep `:thinking` as a suffix; drop other `:variant` tags (`:free`, `:flex`, `:nitro`).
    IF k ~ ':thinking' THEN
        k := regexp_replace(k, ':thinking.*$', '') || '-thinking';
    ELSE
        k := regexp_replace(k, ':.*$', '');
    END IF;
    k := regexp_replace(k,
        '[-_](us|eu|europe|asia|ap|ca|sa|me|af|uk|jp|au|global)[-_]?(east|west|north|south|central|northeast|southeast|northwest|southwest)?[-_]?[0-9]*$',
        '');
    k := regexp_replace(k, '[-_](fp8|fp4|bf16|int4|int8|awq|gptq|tee|batch)$', '');
    k := regexp_replace(k, '[^a-z0-9.]+', '-', 'g');
    -- Single-digit version parts joined by a dash are a dotted version: `4-5` → `4.5`.
    -- Repeated for `x-y-z`; bounded by word edges so dates and sizes are kept.
    k := regexp_replace(k, '(^|-)([0-9])-([0-9])(?=-|$)', '\1\2.\3', 'g');
    k := regexp_replace(k, '(^|-)([0-9])-([0-9])(?=-|$)', '\1\2.\3', 'g');
    RETURN trim(both '-' FROM k);
END
$$;

REINDEX INDEX market_listings_model_key_idx;
REINDEX INDEX market_listings_media_idx;
