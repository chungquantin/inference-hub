-- The market landscape import: roles the first taxonomy had no place for.
--
-- New kinds:
--   decentralized  networks and marketplaces that pool compute from many operators
--   llmops         tools around serving: observability, evals, guardrails, benchmarks
--   agent_tool     coding agents, agent frameworks and harnesses
-- Postgres cannot use a new enum value in the transaction that adds it, so this
-- migration only adds values and rows that do not depend on them.
ALTER TYPE org_kind ADD VALUE IF NOT EXISTS 'decentralized' BEFORE 'investor';
ALTER TYPE org_kind ADD VALUE IF NOT EXISTS 'llmops' BEFORE 'investor';
ALTER TYPE org_kind ADD VALUE IF NOT EXISTS 'agent_tool' BEFORE 'investor';

INSERT INTO categories (slug, name, description, sort_order) VALUES
    ('decentralized-compute', 'Decentralized Compute', 'Networks and marketplaces that pool GPUs from many independent operators.', 55),
    ('llmops',                'LLMOps',                'Observability, evaluation, guardrails and benchmarks around model serving.', 45),
    ('agent-tools',           'Agent Tools',           'Coding agents, agent frameworks and harnesses that consume inference.', 47)
ON CONFLICT (slug) DO NOTHING;

-- Finer groups under the first categories. An organization is listed in a
-- sub-category through `categories:` in its YAML.
INSERT INTO categories (slug, name, description, sort_order, parent_id)
SELECT v.slug, v.name, v.description, v.sort_order, p.id
FROM (VALUES
    ('serverless-gpu',      'Serverless GPU',        'Platforms that run your code or model on GPUs billed per second.',          21, 'inference-providers'),
    ('gpu-marketplaces',    'GPU Marketplaces',      'Spot markets and aggregators reselling GPUs from many clouds.',             51, 'gpu-clouds'),
    ('sovereign-clouds',    'Sovereign & Regional',  'Clouds built to keep AI workloads inside one country or region.',          52, 'gpu-clouds'),
    ('inference-engines',   'Inference Engines',     'Runtimes that execute models: batching, KV cache, kernels, quantization.', 41, 'open-source'),
    ('local-runtimes',      'Local Runtimes',        'Apps and servers that run models on your own machine.',                    42, 'open-source'),
    ('serving-orchestration','Serving & Orchestration','Kubernetes operators, schedulers and routers for model fleets.',          43, 'open-source'),
    ('model-optimization',  'Model Optimization',    'Quantization, speculative decoding, structured output and kernels.',       44, 'open-source'),
    ('observability',       'Observability',         'Tracing, logging and monitoring of LLM calls.',                            46, 'llmops'),
    ('evals-benchmarks',    'Evals & Benchmarks',    'Evaluation frameworks, leaderboards and price/speed benchmarks.',          48, 'llmops'),
    ('guardrails',          'Guardrails',            'Input and output filtering, prompt-injection defense.',                    49, 'llmops')
) AS v(slug, name, description, sort_order, parent)
JOIN categories p ON p.slug = v.parent
ON CONFLICT (slug) DO NOTHING;

-- Names that are also ordinary words or other products ("Jan", "Modal",
-- "Replicate") match news only through their aliases when this is false.
ALTER TABLE organizations ADD COLUMN match_name boolean NOT NULL DEFAULT true;
