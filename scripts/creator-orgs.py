#!/usr/bin/env python3
"""Writes apps/web/src/lib/creator-orgs.json: which organization page belongs to a
catalog creator (`deepseek-ai` -> `deepseek`), so a model's creator links to its page.

A creator maps to an organization when its id, name or an alias equals the
organization's slug, name or a tagging alias (ignoring case and punctuation), or
when it is in OVERRIDES: pairs checked by hand where the names differ but the
company is the same. Nothing else is linked: an unmatched creator keeps its
badge and links to the models it made.
"""
import json, re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OVERRIDES = {  # creator id -> organization slug
    "mistral": "mistral-ai",   # Mistral AI
    "alibaba": "alibaba-cloud",  # Qwen is Alibaba Cloud's model family
    "zhipuai": "z-ai",         # Z.ai is Zhipu AI
    "moonshotai": "moonshot-ai",
    "amazon": "aws",           # Nova models: Amazon Web Services
    "xai": "xai",
    "blackforestlabs": "black-forest-labs",
    "huggingfaceh4": "hugging-face",
}
norm = lambda s: re.sub(r"[^a-z0-9]", "", s.lower())

orgs = {}
for f in (ROOT / "data/organizations").glob("*.yaml"):
    t = f.read_text()
    slug = re.search(r"^slug:\s*(\S+)", t, re.M)[1]
    name = re.search(r'^name:\s*"?([^"\n]+?)"?\s*$', t, re.M)[1]
    al = re.search(r"^\s+aliases:\s*\[(.*?)\]", t, re.M)
    names = [slug, name] + ([x.strip().strip('"') for x in al[1].split(",")] if al else [])
    for n in names:
        orgs.setdefault(norm(n), slug)

creators = json.loads((ROOT / "apps/web/src/lib/catalog.json").read_text())["creators"]
out = {}
for cid, c in creators.items():
    if cid in OVERRIDES:
        out[cid] = OVERRIDES[cid]
        continue
    for key in [cid, c["name"], *c["aliases"]]:
        if norm(key) in orgs:
            out[cid] = orgs[norm(key)]
            break
slugs = {m for m in orgs.values()}
for cid, slug in out.items():
    assert slug in slugs, (cid, slug)
(ROOT / "apps/web/src/lib/creator-orgs.json").write_text(json.dumps(dict(sorted(out.items())), indent=1) + "\n")
print(len(out), "of", len(creators), "creators link to an organization page")
