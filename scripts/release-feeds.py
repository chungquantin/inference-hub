#!/usr/bin/env python3
"""Adds each open-source project's GitHub releases feed as a news feed.

For an organization that has no feed yet and a product with a `github:` repository,
`https://github.com/<owner>/<repo>/releases.atom` is the project's own announcement
channel. It is added when the feed answers, has at least 3 releases and the latest is
less than a year old. Writes data/landscape/release-feeds-report.json.

  python3 scripts/release-feeds.py [today as YYYY-MM-DD]
"""
import json, re, subprocess, sys
from concurrent.futures import ThreadPoolExecutor
from datetime import date, timedelta
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ORGS = ROOT / "data/organizations"
today = date.fromisoformat(sys.argv[1]) if len(sys.argv) > 1 else date.today()

def probe(item):
    slug, repo = item
    p = subprocess.run(["curl", "-sL", "-m", "20", "-A", "InferenceHubBot/0.1", "-w", "\n%{http_code}",
                        f"https://github.com/{repo}/releases.atom"], capture_output=True, text=True)
    body, _, code = p.stdout.rpartition("\n")
    updated = sorted(re.findall(r"<updated>([^<]+)</updated>", body), reverse=True)
    entries = len(re.findall(r"<entry>", body))
    return slug, repo, code, entries, (updated[0][:10] if updated else None)

def main():
    candidates = []
    for f in sorted(ORGS.glob("*.yaml")):
        t = f.read_text()
        if re.search(r"^feeds:", t, re.M):
            continue
        repos = re.findall(r"^\s+github:\s*([\w.-]+/[\w.-]+)\s*$", t, re.M)
        if repos:
            candidates.append((f.stem, repos[0]))
    with ThreadPoolExecutor(12) as ex:
        results = list(ex.map(probe, candidates))
    report = {"written": [], "refused": []}
    for slug, repo, code, entries, latest in results:
        why = None
        if code != "200":
            why = f"HTTP {code}"
        elif entries < 3:
            why = f"only {entries} releases"
        elif not latest or date.fromisoformat(latest) < today - timedelta(days=365):
            why = f"stale: latest release {latest}"
        if why:
            report["refused"].append({"slug": slug, "repo": repo, "why": why})
            continue
        path = ORGS / f"{slug}.yaml"
        text = path.read_text()
        name = re.search(r'^name:\s*"?([^"\n]+?)"?\s*$', text, re.M)[1]
        block = ["feeds:", f"  - slug: {slug}-releases", f"    name: {json.dumps(name + ' releases', ensure_ascii=False)}",
                 f'    url: "https://github.com/{repo}/releases.atom"', "    interval_minutes: 360"]
        lines = text.rstrip("\n").split("\n")
        at = next((i for i, l in enumerate(lines) if l.startswith("products:") or l.startswith("funding:")), len(lines))
        lines[at:at] = block
        path.write_text("\n".join(lines) + "\n")
        report["written"].append({"slug": slug, "repo": repo, "releases": entries, "latest": latest})
    (ROOT / "data/landscape/release-feeds-report.json").write_text(json.dumps(report, indent=1) + "\n")
    print({k: len(v) for k, v in report.items()})
    for r in report["refused"]:
        print("  refused", r["slug"], "-", r["why"])

main()
