-- Base taxonomy and a small set of starter organizations so the site renders
-- something meaningful in development. Bulk data will come from the seed importer.

INSERT INTO categories (slug, name, description, sort_order) VALUES
    ('model-providers',     'Model Providers',     'Labs and companies training and serving foundation models.', 10),
    ('inference-providers', 'Inference Providers', 'Platforms serving open and proprietary models via API.', 20),
    ('llm-gateways',        'LLM Gateways',        'Routers, proxies and exchanges in front of many model APIs.', 30),
    ('open-source',         'Open Source',         'Inference engines, runtimes, quantization and serving tools.', 40),
    ('gpu-clouds',          'GPU Clouds',          'Specialized GPU and accelerator cloud providers.', 50),
    ('hyperscalers',        'Hyperscalers',        'General-purpose cloud providers with AI compute offerings.', 60),
    ('hardware',            'Hardware',            'Chips and accelerators built for training and inference.', 70),
    ('investors',           'Investors',           'Venture firms and funds backing compute and inference.', 80);

INSERT INTO organizations (slug, name, kinds, tagline, website, hq_country, founded_year) VALUES
    ('openai',       'OpenAI',        '{model_provider}',                          'Creator of the GPT model family.',              'https://openai.com',         'US', 2015),
    ('anthropic',    'Anthropic',     '{model_provider}',                          'AI safety company building the Claude models.', 'https://www.anthropic.com',  'US', 2021),
    ('mistral-ai',   'Mistral AI',    '{model_provider}',                          'European lab shipping open and commercial models.', 'https://mistral.ai',     'FR', 2023),
    ('together-ai',  'Together AI',   '{inference_provider,gpu_cloud}',            'Cloud for running and fine-tuning open models.', 'https://www.together.ai',   'US', 2022),
    ('fireworks-ai', 'Fireworks AI',  '{inference_provider}',                      'Fast inference platform for open models.',      'https://fireworks.ai',       'US', 2022),
    ('openrouter',   'OpenRouter',    '{llm_gateway}',                             'Unified API across hundreds of models.',        'https://openrouter.ai',      'US', 2023),
    ('vllm',         'vLLM',          '{open_source}',                             'High-throughput LLM inference and serving engine.', 'https://github.com/vllm-project/vllm', NULL, 2023),
    ('coreweave',    'CoreWeave',     '{gpu_cloud}',                               'Specialized cloud for GPU-accelerated workloads.', 'https://www.coreweave.com', 'US', 2017),
    ('lambda',       'Lambda',        '{gpu_cloud,hardware}',                      'GPU cloud and on-prem AI infrastructure.',      'https://lambda.ai',          'US', 2012),
    ('aws',          'Amazon Web Services', '{hyperscaler,inference_provider,hardware}', 'Cloud provider offering Bedrock and Trainium/Inferentia.', 'https://aws.amazon.com', 'US', 2006),
    ('nvidia',       'NVIDIA',        '{hardware,investor}',                       'Maker of the dominant AI accelerators.',        'https://www.nvidia.com',     'US', 1993),
    ('a16z',         'Andreessen Horowitz', '{investor}',                          'Venture firm with a large AI infrastructure portfolio.', 'https://a16z.com',  'US', 2009);

-- Map orgs to categories based on their kinds.
INSERT INTO organization_categories (organization_id, category_id)
SELECT o.id, c.id
FROM organizations o
JOIN LATERAL unnest(o.kinds) AS k(kind) ON true
JOIN categories c ON c.slug = CASE k.kind
    WHEN 'model_provider'     THEN 'model-providers'
    WHEN 'inference_provider' THEN 'inference-providers'
    WHEN 'llm_gateway'        THEN 'llm-gateways'
    WHEN 'open_source'        THEN 'open-source'
    WHEN 'gpu_cloud'          THEN 'gpu-clouds'
    WHEN 'hyperscaler'        THEN 'hyperscalers'
    WHEN 'hardware'           THEN 'hardware'
    WHEN 'investor'           THEN 'investors'
END
ON CONFLICT DO NOTHING;
