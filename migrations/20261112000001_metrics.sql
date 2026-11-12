-- Metrics: any number that describes a player in the inference market over time.
--
-- Prices live in offerings and market_* tables. Everything else (token market caps,
-- model downloads, serving speed, repository stars) is a metric: one series per
-- (source, subject, metric) with one point per day. A series belongs to an
-- organization when the source's name for the subject maps to one.

CREATE TABLE metric_series (
    id              bigserial PRIMARY KEY,
    metric          text NOT NULL,
    -- 'organization', 'model' or 'project': what the series describes.
    subject_type    text NOT NULL,
    -- The source's id for the subject: a token id, a Hugging Face author, a repository.
    subject_key     text NOT NULL,
    subject_name    text NOT NULL,
    organization_id uuid REFERENCES organizations (id) ON DELETE SET NULL,
    source          text NOT NULL,
    latest_value    double precision NOT NULL,
    latest_day      date NOT NULL,
    UNIQUE (metric, subject_type, subject_key)
);
CREATE INDEX metric_series_rank_idx ON metric_series (metric, latest_value DESC);
CREATE INDEX metric_series_org_idx ON metric_series (organization_id) WHERE organization_id IS NOT NULL;

CREATE TABLE metric_points (
    series_id bigint NOT NULL REFERENCES metric_series (id) ON DELETE CASCADE,
    day       date NOT NULL,
    value     double precision NOT NULL,
    PRIMARY KEY (series_id, day)
);

-- When each metric source last ran, and how it is doing. Failing sources back off.
CREATE TABLE metric_source_state (
    source               text PRIMARY KEY,
    last_run_at          timestamptz,
    last_success_at      timestamptz,
    consecutive_failures integer NOT NULL DEFAULT 0,
    last_error           text,
    series               integer NOT NULL DEFAULT 0
);
