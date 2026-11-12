#!/usr/bin/env python3
"""Writes the feeds found by `discover_feeds` into data/organizations/*.yaml.

Input:  data/landscape/feeds.json  (from `cargo run -p ih-ingest --example discover_feeds`)
Output: a `feeds:` block in each organization file that has an accepted feed, and
        data/landscape/feeds-report.json with every decision and its reason.

A discovered feed is written only when it is the organization's own, current
news: see REJECT for the ones refused by hand, and the rules in `refuse()`.
Items of a dedicated AI organization are published at once (`auto_approve`); items
of a general-purpose company (a cloud, a data platform, an investor) go to the
review queue, where the news triage keeps what is about inference.
"""
import json, re, sys
from datetime import date, timedelta
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ORGS = ROOT / "data/organizations"
today = date.fromisoformat(sys.argv[1]) if len(sys.argv) > 1 else date.today()

# Refused by hand after reading each: slug -> reason.
REJECT = {
    "bitsandbytes": "the feed is Hugging Face's blog, not the project's",
    "torchao": "the feed is the PyTorch blog, not the project's",
    "gpt4all": "the feed is Nomic's company news, not the project's",
    "mlperf-inference": "the feed is MLCommons' blog, which covers every benchmark",
    "ooredoo": "the feed is a sitemap of corporate pages, not news",
    "snowflake": "the feed URL is an unlabelled publishing host with no title",
    "envoy-ai-gateway": "the site redirects to an unrelated domain",
    "gmo-internet-group": "undated group press releases, not about AI",
    "liquid-web": "the feed has no dates",
}
# Companies whose blog is mostly about other things: their items wait for review.
# A GPU cloud, model lab or inference tool writing about itself is on topic, so
# its own blog is published at once, as every organization's own feed is.
REVIEW_KINDS = {"hyperscaler", "investor", "other"}
GENERALISTS = {"databricks", "datadog", "kong", "bifrost", "opik", "microsoft", "digitalocean", "epoch-ai", "mlflow"}

MAX_AGE_DAYS = 365
MIN_ENTRIES = 3


def refuse(slug, feed):
    if slug in REJECT:
        return REJECT[slug]
    if not feed["latest"]:
        return "no dated entries"
    if date.fromisoformat(feed["latest"]) < today - timedelta(days=MAX_AGE_DAYS):
        return f"stale: latest entry {feed['latest']}"
    if feed["entries"] < MIN_ENTRIES:
        return f"only {feed['entries']} entries"
    return None


def kinds_of(text):
    m = re.search(r"^kinds:\s*\[(.*?)\]", text, re.M)
    return {k.strip() for k in m[1].split(",")} if m else set()


def existing_feed_slugs():
    slugs = set()
    for f in ORGS.glob("*.yaml"):
        slugs |= set(re.findall(r"^  - slug:\s*(\S+)\n    name:.*\n    url:", f.read_text(), re.M))
    src = ROOT / "data/sources.yaml"
    if src.exists():
        slugs |= set(re.findall(r"^  - slug:\s*(\S+)", src.read_text(), re.M))
    return slugs


def main():
    found = json.loads((ROOT / "data/landscape/feeds.json").read_text())
    taken = existing_feed_slugs()
    report = {"written": [], "refused": [], "none": []}
    for r in found:
        slug, feed = r["slug"], r["feed"]
        if not feed:
            report["none"].append({"slug": slug, "site": r["site"], "why": (r["tried"] or ["no feed declared or at a common path"])[0]})
            continue
        why = refuse(slug, feed)
        if why:
            report["refused"].append({"slug": slug, "url": feed["url"], "why": why})
            continue
        path = ORGS / f"{slug}.yaml"
        text = path.read_text()
        if re.search(r"^feeds:", text, re.M):
            continue
        name = re.search(r'^name:\s*"?([^"\n]+?)"?\s*$', text, re.M)[1]
        kinds = kinds_of(text)
        review = bool(kinds & REVIEW_KINDS) or slug in GENERALISTS
        fslug = f"{slug}-blog"
        n = 2
        while fslug in taken:
            fslug, n = f"{slug}-blog-{n}", n + 1
        taken.add(fslug)
        block = [
            "feeds:",
            f"  - slug: {fslug}",
            f"    name: {json.dumps(name + ' blog', ensure_ascii=False)}",
            f"    url: {json.dumps(feed['url'])}",
            "    interval_minutes: 180",
        ]
        if review:
            block.append("    auto_approve: false")
        lines = text.rstrip("\n").split("\n")
        at = next((i for i, l in enumerate(lines) if l.startswith("products:") or l.startswith("funding:")), len(lines))
        lines[at:at] = block
        path.write_text("\n".join(lines) + "\n")
        report["written"].append({"slug": slug, "feed": feed["url"], "entries": feed["entries"], "latest": feed["latest"], "how": feed["how"], "auto_approve": not review})
    (ROOT / "data/landscape/feeds-report.json").write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n")
    print({k: len(v) for k, v in report.items()})
    print("review queue:", sum(1 for w in report["written"] if not w["auto_approve"]), "| published at once:", sum(1 for w in report["written"] if w["auto_approve"]))
    for x in report["refused"]:
        print("  refused", x["slug"], "-", x["why"])


main()
