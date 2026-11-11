-- `-turbo` names a different model (whisper-large-v3-turbo is a smaller
-- model than whisper-large-v3), not a deployment of the same one, so it
-- stays in the key. Only quantization and confidential-compute suffixes are
-- dropped.
CREATE OR REPLACE FUNCTION market_model_key(external_id text) RETURNS text
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
    SELECT trim(both '-' FROM regexp_replace(
        regexp_replace(
            regexp_replace(lower(regexp_replace(external_id, '^.*/', '')), ':.*$', ''),
            '[-_](fp8|fp4|bf16|int4|int8|awq|gptq|tee|instruct-fp8)$', ''),
        '[^a-z0-9.]+', '-', 'g'))
$$;

-- Expression indexes depend on the function's result; rebuild them.
REINDEX INDEX market_listings_model_key_idx;
REINDEX INDEX market_listings_media_idx;
