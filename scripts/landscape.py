#!/usr/bin/env python3
"""Turns data/landscape/*.tsv into organization YAMLs and logos.

Inputs (all in data/landscape/):
  platforms.tsv, software.tsv   name <TAB> url <TAB> what <TAB> category, as listed
  verification.json             what was fetched for each row: HTTP status, redirect,
                                page title and description, GitHub repo facts

Outputs:
  data/organizations/<slug>.yaml           new organizations (existing files are never rewritten,
                                           only extended with products under `parents`)
  apps/web/public/logos/<slug>.<ext>       logo, with its source in logos/sources.json
  data/landscape/report.json               every decision and why

Only facts that were fetched or that the list states are written. Nothing is guessed:
no headquarters, founding years or funding. Run from the repo root:

  PYTHONPATH=<dir with pillow> python3 scripts/landscape.py [--no-logos]
"""
import io, json, re, subprocess, sys, urllib.parse as up
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LAND = ROOT / "data/landscape"
ORGS = ROOT / "data/organizations"
LOGOS = ROOT / "apps/web/public/logos"
UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130 Safari/537.36"

# ---------------------------------------------------------------- decisions

# Rows already in the directory under another name: nothing new is written.
EXISTING = {
    "Lambda": "lambda", "Vast.ai": "vast-ai", "Together AI": "together-ai", "Fireworks AI": "fireworks-ai",
    "DeepInfra": "deepinfra", "Novita AI": "novita-ai", "Hyperbolic": "hyperbolic", "Featherless AI": "featherless",
    "Chutes": "chutes", "Atlas Cloud": "atlas-cloud", "Inference.net": "inference-net", "Groq": "groq",
    "Cerebras": "cerebras", "SambaNova": "sambanova", "Mistral": "mistral-ai", "Venice": "venice",
    "Requesty": "requesty", "CoreWeave": "coreweave", "Vultr": "vultr", "Verda (DataCrunch)": "verda",
    "Akash Network": "akash-network", "io.net": "io-net", "Shadeform": "shadeform", "OpenAI": "openai",
    "DeepSeek": "deepseek", "RunPod": "runpod", "Oracle OCI Generative AI": "oracle",
    "Google Vertex AI": "google", "Google AI Studio": "google", "Amazon Bedrock": "aws",
    "Azure AI Foundry": "microsoft", "Azure API Management AI gateway": "microsoft",
    "Vercel AI Gateway": "vercel", "HF Inference": "hugging-face", "Linode / Akamai Cloud": "akamai",
    "Anthropic / Claude Platform on AWS": "anthropic", "OpenRouter": "openrouter", "NVIDIA NIM": "nvidia",
    "Meta Llama API": "meta", "vLLM": "vllm", "SGLang": "sglang", "TensorRT-LLM": "nvidia",
    "llama.cpp": "ggml", "Ollama": "ollama", "TGI": "hugging-face",
}

# Rows that become a product of an existing organization: name -> parent slug.
PRODUCT_OF = {
    "NVIDIA Dynamo": "nvidia", "Triton Inference Server": "nvidia", "NVIDIA TensorRT Model Optimizer": "nvidia",
    "NVIDIA NVLink / NIXL": "nvidia", "genai-perf / AIPerf": "nvidia", "NeMo Guardrails": "nvidia",
    "AIBrix": "vllm", "vLLM production-stack": "vllm", "vLLM Semantic Router": "vllm",
    "Red Hat AI / LLM Compressor": "vllm", "GuideLLM": "vllm", "vLLM Ascend": "vllm",
    "SpecForge": "sglang", "whisper.cpp": "ggml",
    "Text Embeddings Inference": "hugging-face", "Candle": "hugging-face",
    "ONNX Runtime GenAI": "microsoft", "OpenAI Evals": "openai", "Llama Guard / Prompt Guard": "meta",
}
PRODUCT_NAME = {
    "NVIDIA NVLink / NIXL": "NIXL", "genai-perf / AIPerf": "AIPerf", "Red Hat AI / LLM Compressor": "LLM Compressor",
    "Llama Guard / Prompt Guard": "PurpleLlama",
}

# Products grouped under one new organization: product row -> (org slug, org name).
GROUPED = {
    "Mooncake": ("kvcache-ai", "kvcache.ai"), "KTransformers": ("kvcache-ai", "kvcache.ai"),
    "MLC LLM": ("mlc-ai", "MLC AI"), "XGrammar": ("mlc-ai", "MLC AI"),
    "Jan": ("jan", "Jan"), "Cortex": ("jan", "Jan"),
    "BentoML": ("bentoml", "BentoML"), "OpenLLM": ("bentoml", "BentoML"),
    "Modular MAX": ("modular", "Modular"), "MAX / Mojo kernels (Modular platform)": ("modular", "Modular"),
    "Ray Serve": ("anyscale", None), "LLMPerf": ("anyscale", None), "Cog": ("replicate", None),
}

# Rows without a usable result of their own.
SKIP = {
    "Lamini": "site does not resolve (DNS failure)",
    "Mystic AI": "site does not resolve (DNS failure)",
    "Nexa SDK": "the repository moved to qualcomm/GenieX and the product is now called GenieX; the list's entry is out of date",
    "prima.cpp": "GitHub repo Lizonghang/prima.cpp returns 404; only unrelated forks remain",
}

# Name, slug and short forms the list writes as two things.
NAME = {
    "Nebius Token Factory": "Nebius", "SpaceXAI / xAI": "xAI", "Unsloth/Nous Portal": "Nous Research",
    "Alibaba Cloud Model Studio": "Alibaba Cloud", "Databricks Mosaic AI": "Databricks",
    "Snowflake Cortex": "Snowflake", "IBM watsonx.ai": "IBM", "DigitalOcean Gradient": "DigitalOcean",
    "Cloudflare Workers AI": "Cloudflare", "OVHcloud AI Endpoints": "OVHcloud",
    "NexGen Cloud / Hyperstack": "Hyperstack", "Northern Data / Taiga": "Northern Data",
    "T-Systems Industrial AI Cloud": "T-Systems", "Swisscom AI Factory": "Swisscom",
    "Yotta (Shakti Cloud)": "Yotta", "Tata Communications / TCS SuperCompute": "Tata Communications",
    "GMO GPU Cloud": "GMO Internet Group", "Samsung SDS Cloud Platform": "Samsung SDS",
    "SK Telecom GPUaaS": "SK Telecom", "Core42 / G42": "Core42", "Firmus / SMC": "Firmus",
    "Singtel RE:AI": "Singtel", "Bit Digital / WhiteFiber": "WhiteFiber", "Mosaic / Lightning AI": "Lightning AI",
    "Lintasarta GPU Merdeka": "Lintasarta", "Targon (SN4)": "Targon", "Baidu Qianfan": "Baidu AI Cloud",
    "Huawei Cloud ModelArts": "Huawei Cloud", "Volcengine Ark": "Volcengine", "Moonshot AI": "Moonshot AI",
    "MLX / mlx-lm": "MLX", "ExLlamaV2 / V3": "ExLlama", "Bifrost (Maxim)": "Bifrost",
    "LLM Gateway (llmgateway.io)": "LLM Gateway", "Apache APISIX ai-proxy": "Apache APISIX",
    "OpenLLMetry (Traceloop)": "Traceloop", "Arize AX / Phoenix": "Arize AI", "InferenceMAX (SemiAnalysis)": "InferenceMAX",
    "CloudRift benchmark": "CloudRift", "TitanML / Doubleword": "Doubleword", "Deci (NVIDIA)": "Deci",
    "Datadog LLM Observability": "Datadog", "W&B Weave": "Weights & Biases", "MLflow AI Gateway": "MLflow",
    "Comet Opik": "Opik", "Gateway API Inference Extension": "Gateway API Inference Extension",
    "Tyk AI Studio": "Tyk", "Kong AI Gateway": "Kong", "Envoy AI Gateway": "Envoy AI Gateway",
    "LangSmith": "LangChain", "Llama Guard / Prompt Guard": "PurpleLlama", "HIVE Digital": "HIVE Digital",
    "Lakera Guard": "Lakera", "Sify": "Sify", "Darkbloom": "Darkbloom",
}
ALIASES = {
    "Nebius": ["Nebius AI", "Token Factory"], "xAI": ["Grok", "x.ai"], "Hyperstack": ["NexGen Cloud"],
    "Northern Data": ["Taiga Cloud"], "Yotta": ["Shakti Cloud"], "Tata Communications": ["TCS SuperCompute"],
    "WhiteFiber": ["Bit Digital"], "Core42": ["G42"], "Lightning AI": ["PyTorch Lightning"],
    "Modal": ["Modal Labs"], "Beam": ["Beam Cloud"], "Cloudflare": ["Workers AI"], "Databricks": ["Mosaic AI"],
    "Snowflake": ["Snowflake Cortex"], "Modular": ["Modular Inc", "Mojo"], "IBM": ["watsonx"], "DigitalOcean": ["DigitalOcean Gradient"],
    "Alibaba Cloud": ["Model Studio", "Alibaba Cloud Model Studio"], "Volcengine": ["Volcano Engine"],
    "Baidu AI Cloud": ["Qianfan", "Baidu Qianfan"], "Huawei Cloud": ["ModelArts"],
    "OVHcloud": ["OVH"], "Replicate": ["Replicate.com"], "Traceloop": ["OpenLLMetry"],
    "Arize AI": ["Arize Phoenix"], "Weights & Biases": ["W&B", "Weave"], "LangChain": ["LangSmith"],
    "Z.ai": ["Zhipu", "Zhipu AI", "GLM"], "Moonshot AI": ["Kimi"], "MiniMax": ["MiniMax AI"],
    "StepFun": ["Step Fun"], "Aion Labs": ["AionLabs"], "Nous Research": ["Nous Portal"],
    "Black Forest Labs": ["BFL"], "Liquid AI": ["Liquid Foundation Models"], "Perplexity": ["Perplexity AI"],
    "SambaNova": [], "MLX": ["mlx-lm"], "Docker Model Runner": [], "Apache APISIX": ["APISIX"],
    "Kong": ["Kong AI Gateway"], "Tyk": ["Tyk AI Studio"], "Cumulus Labs": [], "Anyscale": ["Ray Serve"],
}
# Names that are ordinary words or belong to other things as well: only aliases match in news.
NO_NAME_MATCH = {
    "Modular", "Modal", "Beam", "Jan", "Cog", "Burn", "Candle", "Outlines", "Medusa", "EAGLE", "Infinity", "Cortex",
    "Petals", "Exo", "Unify", "Martian", "Wafer", "Salad", "Ritual", "Ambient", "Lemonade", "Laminar",
    "Galileo", "Inspect AI", "Morph", "Public AI", "Render Network", "KT", "Replicate", "Koyeb", "Fal",
    "fal", "Ionstream", "Latitude.sh", "Civo", "Sify", "Aruba", "Humain", "Relace", "Respan", "Aethir",
    "Sharon AI", "Soluna", "Cumulus Labs", "Rackspace", "Orange Business", "Epoch AI", "Braintrust",
    "Promptfoo", "Opik", "Ragas", "Lunary", "Cirrascale", "Salad", "Gonka", "Targon", "Nosana", "Mancer",
    "Doubleword", "Reka AI", "Runway", "Decart", "Liquid AI", "Sakana", "Upstage", "Wafer", "Higress",
    "Bifrost", "Arize AI", "Datadog", "Kong", "Tyk", "LangChain", "Weights & Biases", "IBM", "Cloudflare",
    "Snowflake", "Databricks", "DigitalOcean", "Hetzner", "Scaleway", "IONOS", "OVHcloud", "Swisscom",
    "T-Systems", "SK Telecom", "TELUS", "Ooredoo", "Singtel", "Viettel", "Rackspace", "IREN", "Hut 8",
    "Bitdeer", "Alibaba Cloud", "Baidu AI Cloud", "Tencent Cloud", "Huawei Cloud", "Volcengine",
    "Samsung SDS", "Tata Communications", "GMO Internet Group", "Sakura Internet", "SAKURA Internet",
    "Claro Brasil", "Kingsoft Cloud", "Lintasarta", "Leaseweb", "Exoscale", "Infomaniak", "Proact IT", "Outscale",
}
# Tagging by name stays on for these even though they are in the set above: names unique enough.
NAME_MATCH_OK = {"IBM", "Cloudflare", "Snowflake", "Databricks", "DigitalOcean", "Datadog", "Hetzner",
                 "Scaleway", "IONOS", "OVHcloud", "Swisscom", "T-Systems", "Alibaba Cloud", "Tencent Cloud",
                 "Huawei Cloud", "Volcengine", "Baidu AI Cloud", "Samsung SDS", "SK Telecom", "Tata Communications",
                 "Leaseweb", "Exoscale", "Infomaniak", "Rackspace", "Humain", "Bitdeer", "Hut 8", "Kong", "Tyk",
                 "Weights & Biases", "LangChain", "Arize AI", "Higress", "Bifrost", "Upstage", "Decart", "Sakana",
                 "Runway", "Reka AI", "Liquid AI", "IREN", "Singtel", "Viettel", "TELUS", "Ooredoo", "Outscale",
                 "Proact IT", "Kingsoft Cloud", "Lintasarta", "GMO Internet Group", "SAKURA Internet",
                 "Claro Brasil", "Orange Business", "Epoch AI", "Braintrust", "Promptfoo", "Opik", "Cirrascale",
                 "Nosana", "Gonka", "Targon", "Mancer", "Doubleword"}

# GitHub organizations checked by hand: each one's name or homepage matches the company's own site.
# Used for the logo when the site has no usable icon.
VERIFIED_GH_ORG = {
    "modular": "modular", "kserve": "kserve", "mlflow": "mlflow", "predibase": "predibase",
    "openpipe": "OpenPipe", "salad": "SaladTechnologies", "weights-and-biases": "wandb",
    "lunary": "lunary-ai", "minimax": "MiniMax-AI", "moonshot-ai": "MoonshotAI", "genesis-cloud": "GenesisCloud",
}

# Organizations the sites say were acquired or wound down (fetched, see report).
STATUS = {
    "Lepton AI": ("acquired", "lepton.ai now redirects to NVIDIA DGX Cloud Lepton"),
    "OctoAI": ("acquired", "octo.ai redirects to nvidia.com"),
    "Deci": ("acquired", "deci.ai redirects to nvidia.com"),
    "Predibase": ("acquired", "predibase.com redirects to rubrik.com"),
    "OpenPipe": ("acquired", "openpipe.ai states the platform migrated to Weights & Biases and CoreWeave"),
    "Langfuse": ("acquired", "langfuse.com announces Langfuse joins ClickHouse"),
    "Portkey": ("acquired", "portkey.ai announces Palo Alto Networks completed the acquisition"),
    "Lakera": ("acquired", "lakera.ai footer reads (c) Check Point Software Technologies"),
}

# ---------------------------------------------------------------- kinds and categories

PLATFORM_RULES = [  # (substring of the list's category, kinds, extra categories); first match wins
    ("decentralized", ["decentralized"], []),
    ("GPU marketplace", ["gpu_cloud"], ["gpu-marketplaces"]),
    ("EU sovereign", ["gpu_cloud"], ["sovereign-clouds"]),
    ("EU cloud", ["gpu_cloud"], ["sovereign-clouds"]),
    ("regional", ["gpu_cloud"], ["sovereign-clouds"]),
    ("China", ["hyperscaler", "inference_provider"], []),
    ("hyperscaler", ["hyperscaler", "inference_provider"], []),
    ("data platform", ["inference_provider"], []),
    ("custom silicon", ["inference_provider", "hardware"], []),
    ("neocloud/model API", ["gpu_cloud", "inference_provider"], []),
    ("gateway/observability", ["llm_gateway", "llmops"], ["observability"]),
    ("gateway", ["llm_gateway"], []),
    ("model maker", ["model_provider"], []),
    ("serverless GPU", ["inference_provider"], ["serverless-gpu"]),
    ("model API", ["inference_provider"], []),
    ("nonprofit", ["inference_provider"], []),
    ("neocloud", ["gpu_cloud"], []),
    ("GPU cloud", ["gpu_cloud"], []),
    ("cloud", ["gpu_cloud"], []),
]
SOFTWARE_RULES = [
    ("Gateway/obs", ["llm_gateway", "llmops"], ["observability"]),
    ("Gateway", ["llm_gateway"], []),
    ("Router", ["llm_gateway"], []),
    ("Observability", ["llmops"], ["observability"]),
    ("Eval", ["llmops"], ["evals-benchmarks"]),
    ("Benchmark", ["llmops"], ["evals-benchmarks"]),
    ("Price comparison", ["llmops"], ["evals-benchmarks"]),
    ("Guardrails", ["llmops"], ["guardrails"]),
    ("Engine", ["open_source"], ["inference-engines"]),
    ("Local/distributed", ["open_source"], ["local-runtimes"]),
    ("Local runtime", ["open_source"], ["local-runtimes"]),
    ("Distributed", ["open_source"], ["local-runtimes"]),
    ("Orchestration", ["open_source"], ["serving-orchestration"]),
    ("Serving", ["open_source"], ["serving-orchestration"]),
    ("Platform", ["other"], ["serving-orchestration"]),
    ("KV", ["open_source"], ["model-optimization"]),
    ("Kernels", ["open_source"], ["model-optimization"]),
    ("Optimization", ["open_source"], ["model-optimization"]),
    ("Structured output", ["open_source"], ["model-optimization"]),
]


def rule(table, category):
    for key, kinds, cats in table:
        if key.lower() in category.lower():
            return list(kinds), list(cats)
    raise SystemExit(f"no rule for category {category!r}")


# ---------------------------------------------------------------- helpers

def slugify(name):
    s = re.sub(r"[^a-z0-9]+", "-", name.lower().replace("&", " and ")).strip("-")
    return s


def q(s):
    return json.dumps(s, ensure_ascii=False)


def clean_what(what, name):
    """The list's description without source hints and unverified deal claims."""
    parts = [p.strip() for p in what.split(";")]
    parts = [p for p in parts if not re.search(r"(?i)\b(acquired|shut down|joined|archived|deal 20)\b", p)]
    s = "; ".join(parts)
    s = re.sub(r"\s*\((?:[^()]*(?:HF|OpenRouter|HF provider)[^()]*)\)", "", s)
    s = re.sub(r"\s*\((?:OpenRouter[^()]*)\)", "", s)
    s = s.strip(" ;.")
    return (s + ".") if s else None


def good_description(text):
    if not text:
        return None
    t = re.sub(r"\s+", " ", text).strip()
    if len(t) < 30 or len(t) > 320 or re.search(r"(?i)just a moment|attention required|access denied|redirecting", t):
        return None
    if sum(1 for c in t if ord(c) < 128) / len(t) < 0.9:
        return None
    return t


def responds(row):
    s = row.get("site")
    return bool(s) and (200 <= s["code"] < 400 or s["code"] in (403, 522))


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, **kw)


# ---------------------------------------------------------------- logos

def pick_icons(meta):
    """Candidate icon URLs, best first: apple-touch, SVG, largest PNG, then /favicon.ico."""
    if not meta:
        return []
    icons = meta.get("icons") or []

    def score(i):
        rel, href, sizes = i["rel"], i["href"].lower(), i.get("sizes") or ""
        n = max([int(x) for x in re.findall(r"(\d+)x\d+", sizes)] or [0])
        if "apple-touch" in rel:
            return (0, -n)
        if href.split("?")[0].endswith(".svg") or (i.get("type") or "").endswith("svg+xml"):
            return (1, 0)
        return (2, -n)

    urls = [i["href"] for i in sorted(icons, key=score) if not i["href"].startswith("data:")]
    base = meta.get("final") or ""
    if base:
        urls.append(up.urljoin(base, "/favicon.ico"))
    return urls


def download(url):
    r = run(["curl", "-sL", "-m", "25", "-A", UA, "-w", "\n%{http_code}\n%{content_type}", url])
    body, _, tail = r.stdout.rpartition(b"\n")
    body, _, code = body.rpartition(b"\n")
    return body, code.decode(), tail.decode()


def check_image(data):
    """(extension, ok, reason). Raster images must be at least 48px and not blank."""
    if data[:5] == b"<?xml" or b"<svg" in data[:2000]:
        if len(data) > 120_000 or re.search(rb"<script|onload=", data, re.I):
            return None, False, "svg too large or scripted"
        return "svg", True, ""
    try:
        from PIL import Image, ImageStat
    except ImportError:
        return None, False, "pillow missing"
    try:
        im = Image.open(io.BytesIO(data))
        fmt = im.format
        im.load()
    except Exception as e:  # noqa: BLE001
        return None, False, f"not an image: {e}"
    w, h = im.size
    if max(w, h) < 48:
        return None, False, f"too small ({w}x{h})"
    rgba = im.convert("RGBA")
    if rgba.getchannel("A").getextrema()[1] < 40:
        return None, False, "transparent"
    # A flat glyph on a transparent background is a valid logo, so judge the picture
    # as it looks on white and on black: it is blank only if both are uniform.
    spread = []
    for bg in ((255, 255, 255, 255), (0, 0, 0, 255)):
        plate = Image.new("RGBA", rgba.size, bg)
        plate.alpha_composite(rgba)
        spread.append(ImageStat.Stat(plate.convert("L")).stddev[0])
    if max(spread) < 2:
        return None, False, "blank"
    return {"PNG": "png", "JPEG": "jpg", "WEBP": "webp", "ICO": "png", "GIF": "png"}.get(fmt, "png"), True, fmt


def save_png(data):
    from PIL import Image
    im = Image.open(io.BytesIO(data))
    if getattr(im, "n_frames", 1) > 1 or im.format == "ICO":
        best = im
        if im.format == "ICO":
            sizes = im.info.get("sizes") or {im.size}
            im.size = max(sizes)
            best = im
        im = best
    out = io.BytesIO()
    im.convert("RGBA").save(out, "PNG")
    return out.getvalue()


def fetch_logo(slug, row, owner, owner_is_org):
    """Returns (file, source) or None. GitHub org avatar for projects, else the site's own icon."""
    candidates = []
    if owner and owner_is_org:
        candidates.append(f"https://github.com/{owner}.png?size=256")
    candidates += pick_icons(row.get("meta"))
    tried = []
    for url in candidates:
        data, code, ctype = download(url)
        if code != "200" or not data:
            tried.append(f"{url}: HTTP {code}")
            continue
        ext, ok, why = check_image(data)
        if not ok:
            tried.append(f"{url}: {why}")
            continue
        if ext == "png" and (why == "ICO" or why == "GIF"):
            data = save_png(data)
        LOGOS.joinpath(f"{slug}.{ext}").write_bytes(data)
        return f"/logos/{slug}.{ext}", url, tried
    return None, None, tried


# ---------------------------------------------------------------- main

def load_rows():
    probe = {(r["section"], r["name"]): r for r in json.loads((LAND / "verification.json").read_text())}
    rows = []
    for section in ("platforms", "software"):
        for line in (LAND / f"{section}.tsv").read_text().splitlines():
            p = line.split("\t")
            if len(p) >= 4:
                rows.append(probe[(section, p[0])])
    return rows


def main():
    want_logos = "--no-logos" not in sys.argv
    rows = load_rows()
    previous = LAND / "report.json"
    if previous.exists():  # a rerun replaces what the last run generated
        for made in json.loads(previous.read_text()).get("created", []):
            (ORGS / f"{made['slug']}.yaml").unlink(missing_ok=True)
    existing = {p.stem for p in ORGS.glob("*.yaml")}
    existing_hosts = {}
    for p in ORGS.glob("*.yaml"):
        m = re.search(r"^website:\s*(\S+)", p.read_text(), re.M)
        if m:
            existing_hosts[up.urlparse(m[1]).hostname.removeprefix("www.")] = p.stem

    report = {"skipped": [], "merged": [], "products": [], "created": [], "no_logo": [], "warnings": []}
    orgs = {}      # slug -> dict
    products = {}  # parent slug -> [product dicts]

    for row in rows:
        name, section = row["name"], row["section"]
        url = row["url"]
        if name in EXISTING:
            report["merged"].append({"name": name, "into": EXISTING[name]})
            continue
        if name in SKIP:
            report["skipped"].append({"name": name, "why": SKIP[name]})
            continue
        gh = row.get("github") if row.get("github") and "error" not in row["github"] else None
        meta = row.get("meta")
        category = row["cat"]
        if category == "?":  # Darkbloom: the list has no category; its own site says decentralized
            category = "decentralized"
        kinds, cats = rule(PLATFORM_RULES if section == "platforms" else SOFTWARE_RULES, category)
        what = clean_what(row["what"], name)
        desc = good_description((meta or {}).get("desc")) or good_description((gh or {}).get("description"))
        if desc and what and desc.rstrip(".").lower() == what.rstrip(".").lower():
            desc = None

        if section == "software" and gh and name not in GROUPED and name not in PRODUCT_OF:
            pass
        elif section == "platforms" and name == "Darkbloom":
            url = "https://www.darkbloom.ai"
            kinds, cats = ["decentralized"], []
            what = "Decentralized private inference network powered by Apple Silicon Macs."
        elif section == "platforms" and not responds(row):
            report["skipped"].append({"name": name, "why": f"site did not respond (HTTP {row['site']['code'] if row.get('site') else 'n/a'})"})
            continue

        display = NAME.get(name, name)
        if name in PRODUCT_NAME:
            display = PRODUCT_NAME[name]

        # products under existing or grouped organizations
        parent = PRODUCT_OF.get(name) or (GROUPED[name][0] if name in GROUPED else None)
        if parent:
            pname = PRODUCT_NAME.get(name, name)
            product = {"slug": slugify(pname), "name": pname, "type": "oss_project", "tagline": what,
                       "url": url, "github": gh["full"] if gh else None,
                       "license": gh and gh["license"] if gh and gh["license"] not in (None, "NOASSERTION") else None}
            products.setdefault(parent, []).append(product)
            report["products"].append({"name": pname, "parent": parent})
            if name in GROUPED and parent not in existing and parent not in orgs:
                orgs[parent] = {
                    "slug": parent, "name": GROUPED[name][1], "kinds": ["open_source"], "cats": cats,
                    "website": None, "tagline": None, "description": None, "row": row, "gh": gh,
                    "aliases": ALIASES.get(GROUPED[name][1] or parent, []), "status": None,
                    "grouped": True,
                }
            elif name in GROUPED and parent in orgs and orgs[parent].get("grouped"):
                # Only an organization made for the group takes its products' categories;
                # a company from the platforms list keeps its own.
                orgs[parent]["cats"] = sorted(set(orgs[parent]["cats"]) | set(cats))
            continue

        slug = slugify(display)
        if slug in existing or slug in orgs:
            report["warnings"].append(f"{name}: slug {slug} already taken; skipped")
            continue
        host = up.urlparse(url).hostname.removeprefix("www.") if url and url.startswith("http") else ""
        if host in existing_hosts and "github.com" not in host:
            report["warnings"].append(f"{name}: website {host} belongs to existing org {existing_hosts[host]}; check")
        # website: the list's URL, or a project's homepage when it has one that answered
        website = url
        if "github.com" in (url or ""):
            home = (gh or {}).get("homepage")
            website = home if home and meta and meta.get("title") is not None else url
        status = STATUS.get(display)
        if status:
            status = status[0]
        orgs[slug] = {
            "slug": slug, "name": display, "kinds": kinds, "cats": cats, "website": website,
            "tagline": what, "description": desc, "row": row, "gh": gh,
            "aliases": ALIASES.get(display, []), "status": status,
            "product": ({"slug": slugify(name) if slugify(name) != slug else slug + "-engine", "name": name,
                         "github": gh["full"], "license": gh["license"] if gh["license"] not in (None, "NOASSERTION") else None,
                         "url": url} if gh and "github.com" in (url or "") else None),
        }

    # logos, in parallel
    def logo_job(o):
        gh = o["gh"]
        owner = gh["owner"] if gh else VERIFIED_GH_ORG.get(o["slug"])
        is_org = bool(gh and gh["owner_type"] == "Organization") or o["slug"] in VERIFIED_GH_ORG
        for ext in ("svg", "png", "jpg", "webp"):
            if (LOGOS / f"{o['slug']}.{ext}").exists():
                return o["slug"], f"/logos/{o['slug']}.{ext}", None, []
        r = fetch_logo(o["slug"], o["row"], owner, is_org)
        return o["slug"], *r

    sources_path = LOGOS / "sources.json"
    sources = json.loads(sources_path.read_text())
    if want_logos:
        with ThreadPoolExecutor(12) as ex:
            for slug, file, source, tried in ex.map(logo_job, orgs.values()):
                if file:
                    orgs[slug]["logo"] = file
                    if source:
                        sources[slug] = {"file": file, "source": source}
                else:
                    report["no_logo"].append({"slug": slug, "tried": tried})
        sources_path.write_text(json.dumps(dict(sorted(sources.items())), indent=1) + "\n")
    else:
        for o in orgs.values():
            for ext in ("svg", "png", "jpg", "webp"):
                if (LOGOS / f"{o['slug']}.{ext}").exists():
                    o["logo"] = f"/logos/{o['slug']}.{ext}"

    # write YAML
    for slug, o in orgs.items():
        row, gh = o["row"], o["gh"]
        out = [f"slug: {slug}", f"name: {q(o['name'])}", f"kinds: [{', '.join(o['kinds'])}]"]
        if o["tagline"]:
            out.append(f"tagline: {q(o['tagline'])}")
        if o["description"]:
            out.append(f"description: {q(o['description'])}")
        if o["website"]:
            out.append(f"website: {q(o['website'])}")
        if o.get("logo"):
            out.append(f"logo_url: {o['logo']}")
        if o["status"]:
            out.append(f"status: {o['status']}")
        links = {}
        if gh:
            links["github"] = f"https://github.com/{gh['owner']}"
        elif slug in VERIFIED_GH_ORG:
            links["github"] = f"https://github.com/{VERIFIED_GH_ORG[slug]}"
        if links:
            out.append("links:")
            out += [f"  {k}: {q(v)}" for k, v in links.items()]
        if o["cats"]:
            out.append(f"categories: [{', '.join(sorted(set(o['cats'])))}]")
        no_match = o["name"] in NO_NAME_MATCH and o["name"] not in NAME_MATCH_OK
        out.append("tagging:")
        if o["aliases"]:
            out.append(f"  aliases: [{', '.join(q(a) for a in o['aliases'])}]")
        out.append("  match_case: true")
        if no_match:
            out.append("  match_name: false")
        prods = list(products.get(slug, []))
        if o.get("product"):
            prods.insert(0, {"slug": o["product"]["slug"], "name": o["product"]["name"], "type": "oss_project",
                             "tagline": o["tagline"], "url": o["product"]["url"],
                             "github": o["product"]["github"], "license": o["product"]["license"]})
        if prods:
            out.append("products:")
            for p in prods:
                out.append(f"  - slug: {p['slug']}")
                out.append(f"    name: {q(p['name'])}")
                out.append(f"    type: {p['type']}")
                if p.get("github"):
                    out.append(f"    github: {p['github']}")
                if p.get("tagline"):
                    out.append(f"    tagline: {q(p['tagline'])}")
                if p.get("url"):
                    out.append(f"    url: {q(p['url'])}")
                if p.get("license"):
                    out.append("    metadata:")
                    out.append(f"      license: {q(p['license'])}")
        (ORGS / f"{slug}.yaml").write_text("\n".join(out) + "\n")
        report["created"].append({"slug": slug, "kinds": o["kinds"], "logo": o.get("logo")})

    # products under existing organizations: appended when `products:` is the last key
    for parent, plist in products.items():
        if parent in orgs:
            continue
        path = ORGS / f"{parent}.yaml"
        text = path.read_text()
        keys = re.findall(r"^([a-z_]+):", text, re.M)
        if "products" not in keys and keys[-1] not in ("tagging", "links", "feeds"):
            report["warnings"].append(f"{parent}.yaml: unexpected last key {keys[-1]}, products not added")
            continue
        have = set(re.findall(r"^\s+github:\s*(\S+)", text, re.M))
        have_slugs = set(re.findall(r"^  - slug:\s*(\S+)", text, re.M))
        add = []
        for p in plist:
            if (p.get("github") in have) or p["slug"] in have_slugs:
                continue
            add.append(p)
        if not add:
            continue
        if "products" not in keys:
            text = text.rstrip("\n") + "\nproducts:\n"
        elif keys[-1] != "products":
            report["warnings"].append(f"{parent}.yaml: `products` is not the last key, products not added")
            continue
        lines = []
        for p in add:
            lines += [f"  - slug: {p['slug']}", f"    name: {q(p['name'])}", f"    type: {p['type']}"]
            if p.get("github"):
                lines.append(f"    github: {p['github']}")
            if p.get("tagline"):
                lines.append(f"    tagline: {q(p['tagline'])}")
            if p.get("url"):
                lines.append(f"    url: {q(p['url'])}")
            if p.get("license"):
                lines += ["    metadata:", f"      license: {q(p['license'])}"]
        path.write_text(text.rstrip("\n") + "\n" + "\n".join(lines) + "\n")

    (LAND / "report.json").write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n")
    print({k: len(v) for k, v in report.items()})
    for w in report["warnings"]:
        print("WARN", w)


if __name__ == "__main__":
    main()
