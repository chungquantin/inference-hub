-- What is known about a model apart from its price: when it came out, what it can read and write,
-- how much it can take in and put out, whether the weights are open. Filled from models.dev,
-- which groups the same model across providers under one canonical id; `model_key` is that id
-- normalized the way the market's keys are, so the facts join the market's models directly.

CREATE TABLE model_facts (
    model_key         text PRIMARY KEY,
    name              text NOT NULL,
    family            text,
    description       text,
    released_on       date,
    -- "2026-06": the month the training data ends, as the source states it.
    knowledge_cutoff  text,
    open_weights      boolean,
    context_window    integer,
    max_output        integer,
    input_modalities  text[] NOT NULL DEFAULT '{}',
    output_modalities text[] NOT NULL DEFAULT '{}',
    reasoning         boolean,
    tool_call         boolean,
    structured_output boolean,
    attachment        boolean,
    -- Every entry for the model is marked deprecated.
    deprecated        boolean NOT NULL DEFAULT false,
    -- Providers that list it at the source.
    providers_listing integer NOT NULL DEFAULT 0,
    source            text NOT NULL DEFAULT 'models.dev',
    updated_at        timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX model_facts_released_idx ON model_facts (released_on DESC NULLS LAST);
