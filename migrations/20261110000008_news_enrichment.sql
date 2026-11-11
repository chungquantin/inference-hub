-- LLM enrichment of news: a topic, relevance and a short summary per item,
-- and organizations the model found that the keyword tagger missed.

ALTER TYPE news_match ADD VALUE IF NOT EXISTS 'llm';

CREATE TABLE news_enrichment (
    news_item_id uuid PRIMARY KEY REFERENCES news_items (id) ON DELETE CASCADE,
    topic        text NOT NULL,
    -- `core` (about the inference market), `adjacent` or `off_topic`.
    relevance    text NOT NULL CHECK (relevance IN ('core', 'adjacent', 'off_topic')),
    -- The model's suggestion for the review queue: `approve` or `reject`.
    verdict      text NOT NULL CHECK (verdict IN ('approve', 'reject')),
    confidence   real NOT NULL CHECK (confidence BETWEEN 0 AND 1),
    -- Written by the model; shown labelled as an AI summary.
    summary_ai   text,
    model        text NOT NULL,
    cost_usd     numeric(12, 8) NOT NULL DEFAULT 0,
    -- Hash of the prompt template and input, so a changed prompt re-enriches.
    prompt_hash  text NOT NULL,
    raw          jsonb NOT NULL,
    enriched_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX news_enrichment_relevance_idx ON news_enrichment (relevance, verdict);
