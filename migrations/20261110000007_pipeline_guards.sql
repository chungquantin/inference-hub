-- Guards on automated data: prices that look wrong are held for review
-- instead of published, and jobs that exhausted their retries are kept.

-- A price an adapter reported that failed a sanity check. The listing keeps
-- its last good price until an editor accepts or rejects the held one.
CREATE TABLE price_holds (
    id              bigserial PRIMARY KEY,
    adapter         text NOT NULL,
    listing_key     text NOT NULL,
    external_id     text NOT NULL,
    kind            text NOT NULL CHECK (kind IN ('model', 'gpu')),
    name            text NOT NULL,
    -- `out_of_band` (outside the plausible range) or `jump` (more than the
    -- allowed factor away from the last stored price).
    reason          text NOT NULL CHECK (reason IN ('out_of_band', 'jump')),
    price_input_per_mtok  numeric(14, 6),
    price_output_per_mtok numeric(14, 6),
    price_per_hour  numeric(12, 4),
    previous_input_per_mtok  numeric(14, 6),
    previous_output_per_mtok numeric(14, 6),
    previous_per_hour numeric(12, 4),
    gpu_count       smallint,
    detail          text,
    held_at         timestamptz NOT NULL DEFAULT now(),
    last_seen_at    timestamptz NOT NULL DEFAULT now(),
    -- `accepted` stores the price on the next run; `rejected` keeps holding it silently.
    resolution      text CHECK (resolution IN ('accepted', 'rejected')),
    resolved_at     timestamptz,
    UNIQUE (adapter, listing_key)
);
CREATE INDEX price_holds_open_idx ON price_holds (held_at DESC) WHERE resolution IS NULL;

-- Jobs that failed every attempt, kept after the queue prunes them.
CREATE TABLE jobs_failed (
    id          bigserial PRIMARY KEY,
    job_id      bigint NOT NULL,
    kind        text NOT NULL,
    payload     jsonb NOT NULL,
    attempts    integer NOT NULL,
    last_error  text,
    failed_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX jobs_failed_kind_idx ON jobs_failed (kind, failed_at DESC);
