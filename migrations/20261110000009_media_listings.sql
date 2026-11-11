-- Listings priced per unit other than chat tokens: embeddings (per million
-- input tokens), images (per image), video (per second), speech (per
-- character) and transcription (per audio second).

ALTER TABLE market_listings DROP CONSTRAINT market_listings_kind_check;
ALTER TABLE market_listings ADD CONSTRAINT market_listings_kind_check
    CHECK (kind IN ('model', 'gpu', 'embedding', 'image', 'video', 'speech', 'transcription'));

ALTER TABLE market_listings
    -- What `price_per_unit` is per: `mtok_input`, `image`, `video_second`,
    -- `character` or `audio_second`.
    ADD COLUMN unit text,
    ADD COLUMN price_per_unit numeric(18, 10),
    ADD CONSTRAINT market_listings_unit_price CHECK ((unit IS NULL) = (price_per_unit IS NULL));

CREATE INDEX market_listings_media_idx ON market_listings (kind, market_model_key(external_id))
    WHERE kind NOT IN ('model', 'gpu') AND delisted_at IS NULL;

ALTER TABLE price_holds DROP CONSTRAINT price_holds_kind_check;
ALTER TABLE price_holds ADD CONSTRAINT price_holds_kind_check
    CHECK (kind IN ('model', 'gpu', 'embedding', 'image', 'video', 'speech', 'transcription'));
ALTER TABLE price_holds
    ADD COLUMN unit text,
    ADD COLUMN price_per_unit numeric(18, 10),
    ADD COLUMN previous_per_unit numeric(18, 10);
