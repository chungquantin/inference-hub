-- News ingestion: feed sources, a Postgres-backed job queue, and the data the
-- name-matching tagger needs.

-- Sources ------------------------------------------------------------------

ALTER TABLE sources
    ADD COLUMN slug                   text,
    ADD COLUMN url                    text,
    -- The organization a first-party source (a company blog) belongs to.
    -- Its items are tagged with that organization.
    ADD COLUMN organization_id        uuid REFERENCES organizations (id) ON DELETE SET NULL,
    -- Publish items without review. Meant for first-party sources.
    ADD COLUMN auto_approve           boolean NOT NULL DEFAULT false,
    ADD COLUMN fetch_interval_minutes integer NOT NULL DEFAULT 60
        CHECK (fetch_interval_minutes >= 5),
    -- HTTP validators for conditional requests.
    ADD COLUMN etag                   text,
    ADD COLUMN last_modified          text,
    ADD COLUMN last_success_at        timestamptz,
    ADD COLUMN consecutive_failures   integer NOT NULL DEFAULT 0;

UPDATE sources SET slug = 'source-' || id WHERE slug IS NULL;
ALTER TABLE sources ALTER COLUMN slug SET NOT NULL;
ALTER TABLE sources ADD CONSTRAINT sources_slug_key UNIQUE (slug);

-- Organizations: tagging rules ----------------------------------------------

ALTER TABLE organizations
    -- Other names the tagger matches, such as "AWS" or "DeepMind".
    ADD COLUMN aliases         text[] NOT NULL DEFAULT '{}',
    -- Phrases that contain a name but do not mean the organization,
    -- such as "AWS Lambda" for Lambda.
    ADD COLUMN exclude_phrases text[] NOT NULL DEFAULT '{}',
    -- Match the name and aliases case-sensitively, for names that are
    -- also common words ("Meta", "Lambda").
    ADD COLUMN match_case      boolean NOT NULL DEFAULT false;

-- News ---------------------------------------------------------------------

ALTER TABLE news_items
    -- The id the feed gives the entry, kept for debugging.
    ADD COLUMN guid   text,
    ADD COLUMN author text;

CREATE TYPE news_match AS ENUM ('source', 'title', 'body', 'manual');

ALTER TABLE news_item_organizations
    ADD COLUMN matched_by news_match NOT NULL DEFAULT 'manual';

CREATE INDEX news_items_source_idx ON news_items (source_id);

-- Jobs ---------------------------------------------------------------------

CREATE TYPE job_status AS ENUM ('queued', 'running', 'done', 'failed');

CREATE TABLE jobs (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    kind         text NOT NULL,
    payload      jsonb NOT NULL DEFAULT '{}'::jsonb,
    status       job_status NOT NULL DEFAULT 'queued',
    -- At most one queued or running job per key, so a job is never enqueued twice.
    dedupe_key   text,
    run_at       timestamptz NOT NULL DEFAULT now(),
    attempts     integer NOT NULL DEFAULT 0,
    max_attempts integer NOT NULL DEFAULT 5,
    locked_at    timestamptz,
    locked_by    text,
    last_error   text,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX jobs_ready_idx ON jobs (run_at, id) WHERE status = 'queued';
CREATE INDEX jobs_running_idx ON jobs (locked_at) WHERE status = 'running';
CREATE UNIQUE INDEX jobs_dedupe_idx ON jobs (dedupe_key)
    WHERE status IN ('queued', 'running');
CREATE TRIGGER jobs_updated_at BEFORE UPDATE ON jobs
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
