"use client";

import Link from "next/link";
import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { HyperLink } from "@/components/hyper-link";
import { Logo } from "@/components/ui/logo";
import type { DirectoryData, Row, Tab } from "@/components/directory/types";

/* ---------- formatting ---------- */

function money(n: number | null | undefined): string {
  if (n == null) return "—";
  if (n >= 100) return `$${Math.round(n).toLocaleString("en-US")}`;
  if (n >= 1) return `$${n.toFixed(2)}`;
  let t = n.toFixed(4).replace(/0+$/, "");
  if ((t.split(".")[1] ?? "").length < 2) t = n.toFixed(2);
  return `$${t}`;
}
const compact = (n: number | null | undefined) =>
  n == null ? "—" : n >= 1e6 ? `${+(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${Math.round(n / 1e3)}K` : String(n);
const times = (g: number) => `${g >= 10 ? Math.round(g) : g.toFixed(1)}×`;
const median = (a: number[]) => {
  if (!a.length) return null;
  const s = [...a].sort((x, y) => x - y);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

const KIND: Record<string, string> = {
  gpu_cloud: "GPU cloud",
  inference_provider: "Inference provider",
  investor: "Investor",
  hardware: "Hardware",
  model_provider: "Model lab",
  open_source: "Open source",
  hyperscaler: "Hyperscaler",
  llm_gateway: "Gateway",
};

/* ---------- operators and parsing ---------- */

type Cmp = "<" | "<=" | ">" | ">=" | "=" | ":";
type Op = {
  key: string;
  desc: string;
  examples: string[];
  /** Text operators match a substring; numeric ones compare. */
  text?: boolean;
  test: (r: Row, op: Cmp, v: number | string) => boolean;
};
const cmp = (a: number, op: Cmp, v: number | string) => {
  const n = Number(v);
  switch (op) {
    case "<":
      return a < n;
    case "<=":
      return a <= n;
    case ">":
      return a > n;
    case ">=":
      return a >= n;
    default:
      return a === n;
  }
};
const has = (s: string | undefined, v: number | string) => (s ?? "").toLowerCase().includes(String(v));

const OPS: Record<Tab, Op[]> = {
  gpus: [
    { key: "price", desc: "Lowest price per hour", examples: ["price<2", "price>5"], test: (r, o, v) => cmp(r.lo ?? 0, o, v) },
    { key: "clouds", desc: "Number of clouds", examples: ["clouds>=8", "clouds>5"], test: (r, o, v) => cmp(r.clouds ?? 0, o, v) },
    { key: "gap", desc: "Highest vs lowest price", examples: ["gap>5", "gap>=10"], test: (r, o, v) => cmp(r.gap ?? 0, o, v) },
    { key: "at", desc: "Cheapest provider", examples: ["at:runpod", "at:shadeform"], text: true, test: (r, _o, v) => has(r.at, v) },
    { key: "by", desc: "GPU maker", examples: ["by:nvidia", "by:amd"], text: true, test: (r, _o, v) => has(r.producer, v) },
  ],
  models: [
    { key: "price", desc: "Lowest blended price per 1M tokens", examples: ["price<0.1", "price<0.5"], test: (r, o, v) => cmp(r.lo ?? 0, o, v) },
    { key: "providers", desc: "Number of providers", examples: ["providers>=8", "providers>5"], test: (r, o, v) => cmp(r.providers ?? 0, o, v) },
    { key: "ctx", desc: "Context window, 128k ok", examples: ["ctx>=128k", "ctx>=1m"], test: (r, o, v) => cmp(r.ctx ?? 0, o, v) },
    { key: "maker", desc: "Model maker", examples: ["maker:openai", "maker:deepseek"], text: true, test: (r, _o, v) => has(r.maker, v) },
    { key: "cap", desc: "Capability: tools, reasoning, json", examples: ["cap:tools", "cap:reasoning"], text: true, test: (r, _o, v) => (r.caps ?? []).some((c) => c.includes(String(v))) },
    { key: "at", desc: "Cheapest provider", examples: ["at:openrouter"], text: true, test: (r, _o, v) => has(r.at, v) },
  ],
  orgs: [
    {
      key: "kind",
      desc: "Kind of company",
      examples: ["kind:gpu", "kind:investor", "kind:gateway"],
      text: true,
      test: (r, _o, v) => (r.kinds ?? []).some((k) => (KIND[k] ?? k).toLowerCase().includes(String(v)) || k.includes(String(v))),
    },
    { key: "hq", desc: "Headquarters country", examples: ["hq:US"], text: true, test: (r, _o, v) => (r.country ?? "").toLowerCase() === v },
  ],
};

const EXAMPLES: Record<Tab, string[]> = {
  gpus: ["under $2", "h100", "clouds>=8", "gap>10"],
  models: ["under $0.1", "deepseek", "cap:reasoning", "providers>=8", "ctx>=1m"],
  orgs: ["kind:gpu", "kind:investor", "kind:gateway", "open source"],
};

const PLACEHOLDER: Record<Tab, string> = {
  gpus: "Search GPUs, or try under $2 or clouds>=8",
  models: "Search models, or try deepseek or cap:reasoning or ctx>=1m",
  orgs: "Search companies, or try kind:investor",
};

const NOUN: Record<Tab, string> = { gpus: "GPUs", models: "models", orgs: "companies" };
const UNIT: Record<Tab, string> = { gpus: "per GPU-hour", models: "per 1M tokens", orgs: "" };

type Token = { label: string; key: string; op: Cmp; v: number | string; raw: string };
type Parsed = { toks: Token[]; free: string[]; unknown: string[] };

function num(s: string): number {
  const m = s.toLowerCase().replace(/[$,]/g, "").match(/^([\d.]+)\s*([km])?$/);
  if (!m) return NaN;
  return parseFloat(m[1]) * (m[2] === "k" ? 1e3 : m[2] === "m" ? 1e6 : 1);
}

/** Free text, `key:value` / `key>n` filters, and phrases like "under $2". */
function parse(q: string, tab: Tab): Parsed {
  const ops = OPS[tab];
  const toks: Token[] = [];
  let rest = q;
  rest = rest.replace(/\b(?:under|below|less than|cheaper than)\s+\$?([\d.]+\s*[km]?)/gi, (m, n: string) => {
    toks.push({ label: `Price under ${money(num(n))}`, key: "price", op: "<", v: num(n), raw: m });
    return " ";
  });
  rest = rest.replace(/\b(?:over|above|more than)\s+\$?([\d.]+\s*[km]?)/gi, (m, n: string) => {
    toks.push({ label: `Price over ${money(num(n))}`, key: "price", op: ">", v: num(n), raw: m });
    return " ";
  });
  rest = rest.replace(/\b([a-z]+)\s*(>=|<=|>|<|:)\s*(\S+)/gi, (m, k: string, op: string, v: string) => {
    const def = ops.find((o) => o.key === k.toLowerCase());
    if (!def) return m;
    if (def.text) {
      toks.push({ label: `${def.key}: ${v}`, key: def.key, op: ":", v: v.toLowerCase(), raw: m });
      return " ";
    }
    const n = num(v);
    if (Number.isNaN(n)) return m;
    const o = (op === ":" ? "=" : op) as Cmp;
    toks.push({ label: `${def.key} ${o} ${def.key === "price" ? money(n) : def.key === "ctx" ? compact(n) : n}`, key: def.key, op: o, v: n, raw: m });
    return " ";
  });
  const free: string[] = [];
  const unknown: string[] = [];
  for (const w of rest.split(/\s+/).filter(Boolean)) {
    const m = w.match(/^([a-z]+)(>=|<=|>|<|:)/i);
    if (m) unknown.push(m[1].toLowerCase());
    else free.push(w.toLowerCase());
  }
  return { toks, free, unknown };
}

/** Substring first, then an ordered subsequence so "h1oo" still finds "h100". */
function fuzzy(hay: string, needle: string): boolean {
  if (hay.includes(needle)) return true;
  if (needle.length < 3) return false;
  let i = 0;
  for (const c of hay) {
    if (c === needle[i]) i++;
    if (i === needle.length) return true;
  }
  return false;
}

const SYNONYMS: Partial<Record<Tab, Record<string, RegExp>>> = {
  gpus: { blackwell: /^(b\d{2,3}|gb\d{3}|rtx pro 6000)/i },
};

function matches(r: Row, p: Parsed, tab: Tab): boolean {
  for (const t of p.toks) {
    const def = OPS[tab].find((o) => o.key === t.key);
    if (def && !def.test(r, t.op, t.v)) return false;
  }
  const hay = `${r.name} ${r.sub} ${r.maker ?? ""} ${r.producer ?? ""} ${r.at ?? ""} ${(r.kinds ?? []).map((k) => KIND[k] ?? k).join(" ")} ${r.tagline ?? ""} ${(r.caps ?? []).join(" ")}`.toLowerCase();
  for (const w of p.free) {
    const syn = SYNONYMS[tab]?.[w];
    if (syn) {
      if (!syn.test(r.name)) return false;
      continue;
    }
    if (!fuzzy(hay, w)) return false;
  }
  return true;
}

/* ---------- table definitions ---------- */

type Col = {
  k: string;
  h: string;
  num?: boolean;
  /** Sort key; `null` makes the column unsortable. */
  sort?: string | null;
  optional?: boolean;
  hiddenByDefault?: boolean;
  cell: (r: Row, ctx: { min: number; max: number }) => React.ReactNode;
};

const Mono = ({ children, className = "" }: { children: React.ReactNode; className?: string }) => (
  <span className={`font-mono tabular ${className}`}>{children}</span>
);

function RangeBar({ r, ctx }: { r: Row; ctx: { min: number; max: number } }) {
  const lo = r.lo ?? 0;
  const mid = r.mid ?? lo;
  const hi = r.hi ?? mid;
  const a = Math.log(ctx.min);
  const b = Math.log(ctx.max);
  const p = (v: number) => ((Math.log(v) - a) / (b - a)) * 100;
  const l = p(lo);
  return (
    <span
      role="img"
      aria-label={`${money(lo)} to ${money(hi)}, median ${money(mid)}`}
      className="relative inline-block h-3.5 w-[130px] align-middle before:absolute before:inset-x-0 before:top-1.5 before:h-0.5 before:bg-field"
    >
      <span className="absolute top-[5px] h-1 rounded-sm bg-line-strong" style={{ left: `${l}%`, width: `${Math.max(2, p(hi) - l)}%` }} />
      <span className="absolute top-0.5 h-2.5 w-0.5 bg-ink" style={{ left: `${p(mid)}%` }} />
      <span className="absolute top-[3px] -ml-1 size-2 rounded-full bg-lo" style={{ left: `${l}%` }} />
    </span>
  );
}

const COLS: Record<Tab, Col[]> = {
  gpus: [
    { k: "name", h: "GPU", sort: "name", cell: () => null },
    { k: "producer", h: "Made by", sort: "producer", cell: (r) => <HyperLink type="producer" gpuType={r.name} /> },
    { k: "clouds", h: "Clouds", num: true, cell: (r) => <Mono>{r.clouds}</Mono> },
    { k: "lo", h: "Lowest", num: true, cell: (r) => <Mono className="font-medium text-lo">{money(r.lo)}</Mono> },
    { k: "mid", h: "Median", num: true, cell: (r) => <Mono>{money(r.mid)}</Mono> },
    { k: "hi", h: "Highest", num: true, optional: true, cell: (r) => <Mono>{money(r.hi)}</Mono> },
    { k: "range", h: "Price range", sort: "gap", cell: (r, c) => <RangeBar r={r} ctx={c} /> },
    { k: "gap", h: "Gap", num: true, cell: (r) => <Mono>{times(r.gap ?? 1)}</Mono> },
    { k: "at", h: "Cheapest at", cell: (r) => (r.atOrg ? <HyperLink type="org" slug={r.atOrg.slug} name={r.atOrg.name} logoUrl={r.atOrg.logo} /> : "—") },
    { k: "offers", h: "Offers", num: true, optional: true, hiddenByDefault: true, cell: (r) => <Mono>{r.offers}</Mono> },
  ],
  models: [
    { k: "name", h: "Model", sort: "name", cell: () => null },
    { k: "maker", h: "Maker", sort: "maker", cell: (r) => <HyperLink type="creator" maker={r.maker} /> },
    { k: "providers", h: "Providers", num: true, cell: (r) => <Mono>{r.providers}</Mono> },
    { k: "lo", h: "Lowest", num: true, cell: (r) => <Mono className="font-medium text-lo">{money(r.lo)}</Mono> },
    { k: "mid", h: "Median", num: true, cell: (r) => <Mono>{money(r.mid)}</Mono> },
    { k: "gap", h: "Median vs lowest", num: true, cell: (r) => <Mono>{times(r.gap ?? 1)}</Mono> },
    { k: "ctx", h: "Context", num: true, cell: (r) => <Mono>{compact(r.ctx)}</Mono> },
    {
      k: "caps",
      h: "Capabilities",
      sort: null,
      cell: (r) =>
        (r.caps ?? []).slice(0, 3).map((c) => (
          <span key={c} className="mr-1 inline-flex h-[22px] items-center bg-field px-2 text-[12px] text-ink-2">
            {c}
          </span>
        )),
    },
    { k: "at", h: "Cheapest at", cell: (r) => (r.atOrg ? <HyperLink type="org" {...r.atOrg} logoUrl={r.atOrg.logo} /> : "—") },
    { k: "inPrice", h: "Input / 1M", num: true, optional: true, hiddenByDefault: true, cell: (r) => <Mono>{money(r.inPrice)}</Mono> },
    { k: "outPrice", h: "Output / 1M", num: true, optional: true, hiddenByDefault: true, cell: (r) => <Mono>{money(r.outPrice)}</Mono> },
  ],
  orgs: [
    { k: "name", h: "Company", sort: "name", cell: () => null },
    {
      k: "kinds",
      h: "Kind",
      sort: "kinds",
      cell: (r) =>
        (r.kinds ?? []).map((k) => (
          <span key={k} className="mr-1 inline-flex h-[22px] items-center bg-field px-2 text-[12px] text-ink-2">
            {KIND[k] ?? k}
          </span>
        )),
    },
    { k: "tagline", h: "What they do", cell: (r) => <span className="inline-block max-w-[420px] truncate align-bottom">{r.tagline}</span> },
    {
      k: "web",
      h: "Website",
      cell: (r) =>
        r.web ? (
          <a href={`https://${r.web}`} target="_blank" rel="noopener" className="underline underline-offset-4 hover:text-s-7">
            {r.web}
          </a>
        ) : (
          "—"
        ),
    },
    { k: "country", h: "HQ", optional: true, hiddenByDefault: true, cell: (r) => r.country || "—" },
    { k: "founded", h: "Founded", num: true, optional: true, hiddenByDefault: true, cell: (r) => <Mono>{r.founded ?? "—"}</Mono> },
    { k: "status", h: "Status", cell: (r) => <span className="text-lo">{r.status}</span> },
  ],
};

const DEFAULT_SORT: Record<Tab, [string, "asc" | "desc"]> = {
  gpus: ["clouds", "desc"],
  models: ["providers", "desc"],
  orgs: ["name", "asc"],
};

const META: Record<Tab, { title: string; sub: string; histTitle: string }> = {
  gpus: { title: "GPU prices", sub: "Hourly price of each GPU type across clouds and marketplaces, normalized to one GPU.", histTitle: "Lowest price per GPU-hour" },
  models: { title: "Model prices", sub: "Chat models sold by two or more providers. Blended price weighs input 3 to 1 against output.", histTitle: "Lowest blended price per 1M tokens" },
  orgs: { title: "Companies", sub: "Labs, clouds, gateways, hardware makers and the investors behind them.", histTitle: "Companies by kind" },
};

function metricsOf(tab: Tab, rows: Row[]): [string, string, string][] {
  const byLo = [...rows].sort((a, b) => (a.lo ?? 0) - (b.lo ?? 0))[0];
  const byGap = [...rows].sort((a, b) => (b.gap ?? 0) - (a.gap ?? 0))[0];
  const med = median(rows.map((r) => r.lo ?? 0).filter(Boolean));
  if (tab === "gpus")
    return [
      ["GPU types", String(rows.length), `${rows.reduce((s, r) => s + (r.offers ?? 0), 0)} offers`],
      ["Median lowest price", money(med), "per GPU-hour"],
      ["Cheapest", byLo ? money(byLo.lo) : "—", byLo ? `${byLo.name} at ${byLo.at}` : ""],
      ["Widest gap", byGap ? times(byGap.gap ?? 1) : "—", byGap?.name ?? ""],
    ];
  if (tab === "models")
    return [
      ["Models", rows.length.toLocaleString("en-US"), "sold by 2+ providers"],
      ["Median lowest price", money(med), "per 1M tokens"],
      ["Cheapest", byLo ? money(byLo.lo) : "—", byLo ? `${byLo.name} at ${byLo.at}` : ""],
      ["Widest gap", byGap ? times(byGap.gap ?? 1) : "—", byGap ? `${byGap.name}, median vs lowest` : ""],
    ];
  const by = (k: string) => rows.filter((r) => r.kinds?.includes(k)).length;
  return [
    ["Companies", String(rows.length), "in the directory"],
    ["GPU clouds", String(by("gpu_cloud")), "rent GPUs by the hour"],
    ["Inference providers", String(by("inference_provider")), "sell model APIs"],
    ["Investors", String(by("investor")), "fund the market"],
  ];
}

/* ---------- component ---------- */

type Hist = { a?: number; b?: number; cat?: string; label: string };
type Suggestion = { kind: "op"; text: string; desc: string } | { kind: "row"; text: string; row: Row; desc: string };

export function DirectoryApp({ data, initialTab, initialQuery }: { data: DirectoryData; initialTab: Tab; initialQuery: string }) {
  const [tab, setTab] = useState<Tab>(initialTab);
  const [q, setQ] = useState(initialQuery);
  const [sort, setSort] = useState<{ k: string; dir: "asc" | "desc" } | null>(null);
  const [page, setPage] = useState(1);
  const [per, setPer] = useState(25);
  const [dense, setDense] = useState(false);
  const [shown, setShown] = useState<Record<string, boolean>>({});
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [hist, setHist] = useState<Hist | null>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [colMenu, setColMenu] = useState(false);
  const [flash, setFlash] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const rows = data[tab];
  const parsed = useMemo(() => parse(q, tab), [q, tab]);

  // Keep the address bar in step, so a search can be shared.
  useEffect(() => {
    const u = new URL(window.location.href);
    u.searchParams.set("tab", tab);
    if (q.trim()) u.searchParams.set("q", q);
    else u.searchParams.delete("q");
    window.history.replaceState(null, "", u);
  }, [tab, q]);

  // "/" and Cmd+K focus the search.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (document.activeElement as HTMLElement | null)?.tagName ?? "";
      if ((e.key === "/" || ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k")) && !/INPUT|SELECT|TEXTAREA/.test(tag)) {
        e.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    };
    const onDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setColMenu(false);
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onDown);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onDown);
    };
  }, []);

  const base = useMemo(() => rows.filter((r) => matches(r, parsed, tab)), [rows, parsed, tab]);

  const histVal = (r: Row) => r.lo ?? 0;
  const filtered = useMemo(() => {
    let out = base;
    if (hist) {
      const h = hist;
      out = out.filter((r) => (h.cat ? r.kinds?.includes(h.cat) : histVal(r) >= (h.a ?? 0) && histVal(r) < (h.b ?? Infinity)));
    }
    const [dk, dd] = DEFAULT_SORT[tab];
    const k = sort?.k ?? dk;
    const dir = sort ? sort.dir : dd;
    const key = (k === "range" ? "gap" : k) as keyof Row;
    return [...out].sort((a, b) => {
      let x = a[key] as unknown;
      let y = b[key] as unknown;
      if (Array.isArray(x)) x = x.join();
      if (Array.isArray(y)) y = y.join();
      if (x == null) return 1;
      if (y == null) return -1;
      const c = typeof x === "string" ? x.localeCompare(y as string) : (x as number) - (y as number);
      return dir === "asc" ? c : -c;
    });
  }, [base, hist, sort, tab]);

  const pages = Math.max(1, Math.ceil(filtered.length / per));
  const cur = Math.min(page, pages);
  const start = (cur - 1) * per;
  const view = filtered.slice(start, start + per);

  const ctx = useMemo(() => {
    const all = data[tab];
    return {
      min: Math.min(...all.map((r) => r.lo ?? Infinity).filter(Number.isFinite), 1),
      max: Math.max(...all.map((r) => r.hi ?? r.mid ?? 0), 1),
    };
  }, [data, tab]);

  const cols = COLS[tab].filter((c) => (c.hiddenByDefault ? shown[c.k] === true : shown[c.k] !== false));
  const sortKey = sort?.k ?? DEFAULT_SORT[tab][0];
  const sortDir = sort ? sort.dir : DEFAULT_SORT[tab][1];

  /* suggestions */
  const suggestions = useMemo<Suggestion[]>(() => {
    const ops = OPS[tab];
    const last = (q.split(/\s+/).pop() ?? "").toLowerCase();
    const out: Suggestion[] = [];
    if (!q.trim()) {
      ops.slice(0, 4).forEach((o) => out.push({ kind: "op", text: o.examples[0], desc: o.desc }));
      [...rows]
        .sort((a, b) => (b.clouds ?? b.providers ?? 0) - (a.clouds ?? a.providers ?? 0))
        .slice(0, 4)
        .forEach((r) => out.push({ kind: "row", text: r.name, row: r, desc: r.sub }));
      return out;
    }
    if (last && /^[a-z]+$/.test(last))
      ops
        .filter((o) => (o.key.startsWith(last) || o.desc.toLowerCase().includes(last)) && o.key !== last)
        .slice(0, 3)
        .forEach((o) => out.push({ kind: "op", text: o.examples[0], desc: o.desc }));
    filtered.slice(0, 6).forEach((r) =>
      out.push({
        kind: "row",
        text: r.name,
        row: r,
        desc: tab === "gpus" ? `${money(r.lo)} to ${money(r.hi)} / hr` : tab === "models" ? `${money(r.lo)} / 1M, ${r.providers} providers` : (r.kinds ?? []).map((k) => KIND[k]).join(", "),
      }),
    );
    return out;
  }, [q, tab, rows, filtered]);

  const choose = (s: Suggestion | undefined) => {
    if (!s) return;
    if (s.kind === "op") {
      const parts = q.split(/\s+/);
      if (/^[a-z]+$/i.test(parts[parts.length - 1] ?? "")) parts.pop();
      setQ(`${parts.join(" ")} ${s.text} `.trimStart());
      setPage(1);
      inputRef.current?.focus();
    } else {
      setQ("");
      setHist(null);
      setSort(null);
      setOpen(false);
      const [dk, dd] = DEFAULT_SORT[tab];
      // Land on the page that holds the row, in default order.
      const ordered = [...data[tab]].sort((a, b) => {
        const x = a[dk as keyof Row] as number | string | undefined;
        const y = b[dk as keyof Row] as number | string | undefined;
        if (x == null || y == null) return 0;
        const c = typeof x === "string" ? x.localeCompare(y as string) : x - (y as number);
        return dd === "asc" ? c : -c;
      });
      setPage(Math.floor(Math.max(0, ordered.findIndex((r) => r.id === s.row.id)) / per) + 1);
      setFlash(s.row.id);
      setTimeout(() => document.querySelector(`[data-row="${CSS.escape(s.row.id)}"]`)?.scrollIntoView({ block: "center", behavior: "smooth" }), 60);
      setTimeout(() => setFlash(null), 1800);
    }
  };

  const switchTab = (t: Tab) => {
    setTab(t);
    setQ("");
    setSort(null);
    setHist(null);
    setPage(1);
    setSelected(new Set());
    setShown({});
    setOpen(false);
  };

  const removeToken = (t: Token) => {
    setQ(q.replace(t.raw, " ").replace(/\s+/g, " ").trimStart());
    setPage(1);
  };

  const exportCsv = () => {
    const use = cols.filter((c) => c.k !== "range" && c.k !== "caps");
    const val = (r: Row, k: string) => {
      const v = (r as Record<string, unknown>)[k];
      return Array.isArray(v) ? v.join("; ") : (v ?? "");
    };
    const esc = (s: unknown) => `"${String(s).replace(/"/g, '""')}"`;
    const text = [use.map((c) => esc(c.h)).join(","), ...filtered.map((r) => use.map((c) => esc(val(r, c.k))).join(","))].join("\n");
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([text], { type: "text/csv" }));
    a.download = `${tab}.csv`;
    a.click();
  };

  const metrics = metricsOf(tab, filtered);
  const meta = META[tab];

  return (
    <div className="flex flex-col">
      {/* tabs */}
      <div role="tablist" aria-label="Directory" className="mb-8 flex gap-1 border-b border-line">
        {(["gpus", "models", "orgs"] as Tab[]).map((t) => (
          <button
            key={t}
            role="tab"
            aria-selected={t === tab}
            onClick={() => switchTab(t)}
            className={`relative flex h-11 items-center gap-2 px-3.5 text-[15px] font-medium ${t === tab ? "text-ink after:absolute after:inset-x-2.5 after:-bottom-px after:h-0.5 after:bg-ink" : "text-ink-2 hover:text-ink"}`}
          >
            {t === "gpus" ? "GPUs" : t === "models" ? "Models" : "Companies"}
            <span className="bg-field px-1.5 font-mono text-[12px] text-ink-3">
              {data[t].length}
              {t === "models" ? "+" : ""}
            </span>
          </button>
        ))}
      </div>

      <h1 className="mb-1 font-display text-[36px] leading-10 font-medium tracking-[-0.045em] sm:text-[44px] sm:leading-[48px]">{meta.title}</h1>
      <p className="mb-6 max-w-[70ch] text-[17px] text-ink-2">{meta.sub}</p>

      {/* smart search */}
      <div className="relative mb-3">
        <div className="flex h-[52px] items-center gap-2.5 border border-line-strong bg-wash px-3.5 focus-within:border-s-7 focus-within:ring-[3px] focus-within:ring-s-2">
          <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden className="shrink-0 text-ink-3">
            <circle cx="8" cy="8" r="5.5" stroke="currentColor" strokeWidth="1.6" />
            <path d="M12.2 12.2L16 16" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
          </svg>
          <input
            ref={inputRef}
            type="text"
            value={q}
            autoComplete="off"
            spellCheck={false}
            role="combobox"
            aria-expanded={open}
            aria-controls="dir-suggest"
            aria-autocomplete="list"
            aria-label="Search the directory"
            placeholder={PLACEHOLDER[tab]}
            onChange={(e) => {
              setQ(e.target.value);
              setPage(1);
              setOpen(true);
              setActive(0);
            }}
            onFocus={() => {
              setOpen(true);
              setActive(0);
            }}
            onBlur={() => setTimeout(() => setOpen(false), 120)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setOpen(true);
                setActive((a) => Math.min(suggestions.length - 1, a + 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setActive((a) => Math.max(0, a - 1));
              } else if (e.key === "Enter") {
                const s = suggestions[active];
                if (open && s && (s.kind === "op" || !q.trim() || s.kind === "row")) {
                  e.preventDefault();
                  choose(s);
                }
              } else if (e.key === "Tab" && open && suggestions[active]?.kind === "op") {
                e.preventDefault();
                choose(suggestions[active]);
              } else if (e.key === "Escape") {
                setOpen(false);
                inputRef.current?.blur();
              }
            }}
            className="h-full min-w-0 flex-1 bg-transparent text-[16px] outline-none placeholder:text-ink-3"
          />
          {q ? (
            <button
              onClick={() => {
                setQ("");
                setPage(1);
                inputRef.current?.focus();
              }}
              aria-label="Clear search"
              className="grid size-7 place-items-center text-ink-3 hover:bg-field hover:text-ink"
            >
              <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden>
                <path d="M2 2l10 10M12 2L2 12" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
              </svg>
            </button>
          ) : (
            <kbd className="border border-line-strong bg-paper px-1.5 font-mono text-[12px] text-ink-3">/</kbd>
          )}
        </div>
        {open && (
          <div id="dir-suggest" role="listbox" className="absolute inset-x-0 top-[58px] z-20 max-h-[420px] overflow-auto border border-line-strong bg-wash p-1.5 shadow-[0_18px_40px_-14px_rgba(31,30,26,0.28)]">
            {suggestions.length === 0 && (
              <p className="px-3 py-3 text-[14px] text-ink-3">
                {parsed.unknown[0]
                  ? `“${parsed.unknown[0]}” is not a filter for ${NOUN[tab]}. Try ${OPS[tab][0].examples[0]}.`
                  : `No match yet. Try fewer words, or a filter such as ${OPS[tab][0].examples[0]}.`}
              </p>
            )}
            {suggestions.map((s, i) => {
              const prev = suggestions[i - 1];
              const heading = !prev || prev.kind !== s.kind ? (s.kind === "op" ? (q.trim() ? "Filters" : "Try a filter") : q.trim() ? "Jump to" : "Or search for") : null;
              return (
                <Fragment key={`${s.kind}-${s.text}-${i}`}>
                  {heading && <h3 className="mx-2.5 mt-2 mb-1 text-[12px] font-semibold text-ink-3">{heading}</h3>}
                  <button
                    role="option"
                    aria-selected={i === active}
                    onMouseDown={(e) => {
                      e.preventDefault();
                      choose(s);
                    }}
                    onMouseEnter={() => setActive(i)}
                    className={`flex w-full items-center gap-3 px-2.5 py-2 text-left ${i === active ? "bg-s-1" : ""}`}
                  >
                    {s.kind === "op" ? (
                      <code className="bg-field px-1.5 font-mono text-[13px]">{s.text}</code>
                    ) : (
                      <Logo name={s.row.name} src={s.row.logo} size="sm" />
                    )}
                    <span className="min-w-0 flex-1 truncate">{s.kind === "op" ? "" : s.text}</span>
                    <span className="truncate text-[13px] text-ink-3">{s.desc}</span>
                  </button>
                </Fragment>
              );
            })}
            <p className="mt-1.5 flex flex-wrap gap-4 border-t border-line px-3 pt-2.5 pb-1 text-[13px] text-ink-3">
              <span>↑ ↓ move</span>
              <span>Enter pick</span>
              <span>Esc close</span>
            </p>
          </div>
        )}
      </div>

      {/* chips */}
      <div className="mb-5 flex min-h-8 flex-wrap items-center gap-2" aria-live="polite">
        {parsed.toks.map((t) => (
          <span key={t.raw} className="inline-flex h-[30px] items-center gap-1.5 border border-s-3 bg-s-2 pr-1.5 pl-2.5 text-[13px] font-medium text-s-9">
            {t.label}
            <button onClick={() => removeToken(t)} aria-label={`Remove filter ${t.label}`} className="grid size-5 place-items-center hover:bg-s-3/50">
              <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
                <path d="M1 1l8 8M9 1L1 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </button>
          </span>
        ))}
        {parsed.unknown.map((u) => (
          <span key={u} className="inline-flex h-[30px] items-center border border-[#ecd49a] bg-warn-tint px-2.5 text-[13px] font-medium text-warn">
            “{u}” is not a filter for {NOUN[tab]}, so it was ignored
          </span>
        ))}
        {hist && (
          <span className="inline-flex h-[30px] items-center gap-1.5 border border-s-3 bg-s-2 pr-1.5 pl-2.5 text-[13px] font-medium text-s-9">
            {hist.label}
            <button onClick={() => setHist(null)} aria-label="Remove range filter" className="grid size-5 place-items-center hover:bg-s-3/50">
              <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
                <path d="M1 1l8 8M9 1L1 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </button>
          </span>
        )}
        {!parsed.toks.length && !hist && !parsed.unknown.length && (
          <>
            <span className="text-[13px] text-ink-3">Try</span>
            {EXAMPLES[tab].map((e) => (
              <button
                key={e}
                onClick={() => {
                  setQ(e);
                  setPage(1);
                }}
                className="h-[30px] border border-line-strong bg-wash px-2.5 text-[13px] text-ink-2 hover:border-ink hover:text-ink"
              >
                {e}
              </button>
            ))}
          </>
        )}
      </div>

      {/* metrics */}
      <section aria-label="Summary of the results below" className="mb-5 grid overflow-hidden border border-line-strong bg-wash lg:grid-cols-[minmax(0,5fr)_minmax(0,6fr)]">
        <div className="grid grid-cols-2">
          {metrics.map(([l, v, s], i) => (
            <div key={l} className={`min-w-0 border-line p-4 ${i % 2 === 0 ? "border-r" : ""} ${i < 2 ? "border-b" : ""}`}>
              <span className="block text-[13px] text-ink-3">{l}</span>
              <span className="block truncate text-[26px] leading-[34px] font-medium tracking-[-0.03em]">{v}</span>
              <span className="block truncate text-[13px] text-ink-2">{s}</span>
            </div>
          ))}
        </div>
        <Histogram tab={tab} rows={base} all={rows} hist={hist} onPick={(h) => { setHist(h); setPage(1); }} title={meta.histTitle} />
      </section>

      {/* toolbar */}
      <div className="mb-2.5 flex flex-wrap items-center gap-2.5">
        <p className="mr-auto text-[14px] text-ink-2" aria-live="polite">
          <b className="font-semibold text-ink">{filtered.length.toLocaleString("en-US")}</b> of {rows.length.toLocaleString("en-US")} {NOUN[tab]}
          {UNIT[tab] ? `, prices ${UNIT[tab]}` : ""}
        </p>
        <div role="group" aria-label="Row height" className="inline-flex border border-line-strong">
          {[
            ["Comfortable", false],
            ["Compact", true],
          ].map(([label, v]) => (
            <button key={String(label)} aria-pressed={dense === v} onClick={() => setDense(v as boolean)} className={`h-8 px-3 text-[14px] ${dense === v ? "bg-ink text-wash" : "bg-wash text-ink-2 hover:text-ink"}`}>
              {label as string}
            </button>
          ))}
        </div>
        <div ref={menuRef} className="relative">
          <button
            aria-expanded={colMenu}
            aria-haspopup="true"
            onClick={() => setColMenu((o) => !o)}
            className={`h-9 border border-line-strong px-3 text-[14px] font-medium hover:border-ink ${colMenu ? "bg-paper" : "bg-wash"}`}
          >
            Columns
          </button>
          {colMenu && (
            <div className="absolute top-10 right-0 z-30 min-w-[210px] border border-line-strong bg-wash p-2 shadow-[0_18px_40px_-14px_rgba(31,30,26,0.28)]">
              {COLS[tab]
                .filter((c) => c.k !== "name")
                .map((c) => {
                  const on = c.hiddenByDefault ? shown[c.k] === true : shown[c.k] !== false;
                  return (
                    <label key={c.k} className="flex cursor-pointer items-center gap-2.5 px-2 py-1.5 text-[14px] hover:bg-paper">
                      <input type="checkbox" checked={on} onChange={(e) => setShown((s) => ({ ...s, [c.k]: e.target.checked }))} className="size-4 accent-s-7" />
                      {c.h}
                    </label>
                  );
                })}
            </div>
          )}
        </div>
        <button onClick={exportCsv} className="h-9 border border-line-strong bg-wash px-3 text-[14px] font-medium hover:border-ink">
          Export CSV
        </button>
      </div>

      {/* table */}
      <div className="max-h-[min(70vh,760px)] overflow-auto border border-line-strong bg-wash">
        <table className="w-full border-separate border-spacing-0 text-[14px]">
          <thead>
            <tr>
              <th scope="col" className="sticky top-0 left-0 z-[4] h-10 w-[42px] border-b border-line bg-paper pl-3.5 text-left">
                <input
                  type="checkbox"
                  aria-label="Select all rows on this page"
                  checked={view.length > 0 && view.every((r) => selected.has(r.id))}
                  onChange={(e) =>
                    setSelected((s) => {
                      const n = new Set(s);
                      view.forEach((r) => (e.target.checked ? n.add(r.id) : n.delete(r.id)));
                      return n;
                    })
                  }
                  className="size-4 accent-s-7"
                />
              </th>
              {cols.map((c) => {
                const key = c.sort === null ? null : (c.sort ?? c.k);
                const on = key != null && sortKey === key;
                const first = c.k === "name";
                return (
                  <th
                    key={c.k}
                    scope="col"
                    aria-sort={on ? (sortDir === "asc" ? "ascending" : "descending") : "none"}
                    className={`sticky top-0 h-10 border-b border-line bg-paper px-3.5 text-[13px] font-medium whitespace-nowrap ${c.num ? "text-right" : "text-left"} ${first ? "left-[42px] z-[4] shadow-[1px_0_0_var(--line)]" : "z-[3]"} ${on ? "text-ink" : "text-ink-2"}`}
                  >
                    {key ? (
                      <button
                        onClick={() => {
                          if (on) setSort({ k: key, dir: sortDir === "asc" ? "desc" : "asc" });
                          else setSort({ k: key, dir: key === "name" || key === "kinds" || key === "at" || key === "producer" || key === "maker" || key === "lo" || key === "mid" ? "asc" : "desc" });
                          setPage(1);
                        }}
                        className="inline-flex items-center gap-1.5 hover:text-ink"
                      >
                        {c.h}
                        <span aria-hidden className="w-2.5 text-[10px]">
                          {on ? (sortDir === "asc" ? "▲" : "▼") : ""}
                        </span>
                      </button>
                    ) : (
                      c.h
                    )}
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {view.map((r) => {
              const sel = selected.has(r.id);
              const bg = sel ? "bg-s-1" : flash === r.id ? "bg-mark" : "bg-wash group-hover:bg-ground";
              const cell = `border-b border-line ${bg} ${dense ? "h-[34px]" : "h-11"}`;
              return (
                <tr key={r.id} data-row={r.id} aria-selected={sel} className="group">
                  <td className={`sticky left-0 z-[2] pl-3.5 ${cell}`}>
                    <input
                      type="checkbox"
                      aria-label={`Select ${r.name}`}
                      checked={sel}
                      onChange={(e) =>
                        setSelected((s) => {
                          const n = new Set(s);
                          if (e.target.checked) n.add(r.id);
                          else n.delete(r.id);
                          return n;
                        })
                      }
                      className="size-4 accent-s-7"
                    />
                  </td>
                  {cols.map((c) =>
                    c.k === "name" ? (
                      <td key={c.k} className={`sticky left-[42px] z-[2] px-3.5 whitespace-nowrap shadow-[1px_0_0_var(--line)] ${cell}`}>
                        <Link href={r.href} className="flex min-w-[180px] items-center gap-2.5">
                          <Logo name={r.name} src={r.logo} size="md" />
                          <span className="min-w-0 max-w-[260px]">
                            <span className="block truncate font-medium hover:text-s-7 hover:underline">
                              <Highlight text={r.name} words={parsed.free} />
                            </span>
                            {!dense && r.sub && <span className="block truncate text-[12px] leading-[14px] text-ink-3">{r.sub}</span>}
                          </span>
                        </Link>
                      </td>
                    ) : (
                      <td key={c.k} className={`px-3.5 whitespace-nowrap ${c.num ? "text-right" : "text-left"} ${cell}`}>
                        {c.cell(r, ctx)}
                      </td>
                    ),
                  )}
                </tr>
              );
            })}
          </tbody>
        </table>
        {view.length === 0 && (
          <div className="px-6 py-14 text-center">
            <b className="mb-1 block text-[17px]">No {NOUN[tab]} match</b>
            <p className="mb-4 text-ink-2">
              {parsed.unknown[0]
                ? `“${parsed.unknown[0]}” is not a filter for ${NOUN[tab]}. Use ${OPS[tab].map((o) => o.key).join(", ")}.`
                : parsed.toks.length || hist
                  ? "Remove a filter or widen the range."
                  : "Check the spelling, or search by maker or provider."}
            </p>
            <button
              onClick={() => {
                setQ("");
                setHist(null);
              }}
              className="h-9 border border-line-strong bg-wash px-3 text-[14px] font-medium hover:border-ink"
            >
              Clear search
            </button>
          </div>
        )}
      </div>

      {/* pager */}
      <div className="mt-3 flex flex-wrap items-center justify-between gap-3 text-[14px] text-ink-2">
        <span>{filtered.length ? `${start + 1}–${Math.min(start + per, filtered.length)} of ${filtered.length}` : ""}</span>
        {pages > 1 && (
          <nav aria-label="Pages" className="flex items-center gap-1.5">
            <PageBtn disabled={cur <= 1} onClick={() => setPage(cur - 1)}>
              Prev
            </PageBtn>
            {pageList(cur, pages).map((p, i) =>
              p === 0 ? (
                <span key={`gap-${i}`} aria-hidden>
                  …
                </span>
              ) : (
                <PageBtn key={p} current={p === cur} onClick={() => setPage(p)}>
                  {p}
                </PageBtn>
              ),
            )}
            <PageBtn disabled={cur >= pages} onClick={() => setPage(cur + 1)}>
              Next
            </PageBtn>
          </nav>
        )}
        <label className="flex items-center gap-2">
          Rows
          <select
            value={per}
            onChange={(e) => {
              setPer(+e.target.value);
              setPage(1);
            }}
            className="h-9 border border-line-strong bg-wash px-2"
          >
            {[15, 25, 50, 100].map((n) => (
              <option key={n}>{n}</option>
            ))}
          </select>
        </label>
      </div>

      {/* selection bar */}
      <div
        role="region"
        aria-label="Selection"
        className={`fixed bottom-5 left-1/2 z-40 flex -translate-x-1/2 items-center gap-4 bg-charcoal py-2.5 pr-3 pl-5 text-wash shadow-[0_18px_40px_-12px_rgba(31,30,26,0.5)] transition-transform ${selected.size ? "translate-y-0" : "translate-y-24"}`}
        aria-hidden={selected.size === 0}
      >
        <span className="text-[14px]">{selected.size} selected</span>
        <Link
          href={selected.size >= 2 && tab === "models" ? `/market/models/${encodeURIComponent([...selected][0])}` : "#"}
          aria-disabled={selected.size < 2}
          tabIndex={selected.size < 2 ? -1 : 0}
          onClick={(e) => selected.size < 2 && e.preventDefault()}
          className={`inline-flex h-9 items-center bg-wash px-3.5 text-[14px] font-medium text-ink ${selected.size < 2 ? "opacity-50" : ""}`}
        >
          {selected.size >= 2 ? `Compare ${selected.size}` : "Select 2 to compare"}
        </Link>
        <button onClick={() => setSelected(new Set())} className="h-9 border border-[#55554e] px-3.5 text-[14px] hover:bg-white/10">
          Clear
        </button>
      </div>
    </div>
  );
}

function PageBtn({ children, onClick, disabled, current }: { children: React.ReactNode; onClick: () => void; disabled?: boolean; current?: boolean }) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      aria-current={current ? "page" : undefined}
      className={`h-9 min-w-9 border px-2.5 disabled:opacity-40 ${current ? "border-ink bg-ink text-wash" : "border-line-strong bg-wash hover:border-ink disabled:hover:border-line-strong"}`}
    >
      {children}
    </button>
  );
}

/** Page numbers with gaps: 1 … 4 5 6 … 20. A 0 marks a gap. */
function pageList(cur: number, pages: number): number[] {
  const set = new Set([1, pages, cur, cur - 1, cur + 1].filter((n) => n >= 1 && n <= pages));
  const out: number[] = [];
  let prev = 0;
  for (const n of [...set].sort((a, b) => a - b)) {
    if (n - prev > 1) out.push(0);
    out.push(n);
    prev = n;
  }
  return out;
}

function Highlight({ text, words }: { text: string; words: string[] }) {
  const w = words.filter((x) => x.length >= 2);
  if (!w.length) return <>{text}</>;
  const re = new RegExp(`(${w.map((x) => x.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})`, "ig");
  return (
    <>
      {text.split(re).map((part, i) =>
        i % 2 ? (
          <mark key={i} className="rounded-[2px] bg-[#fff0a8] px-px text-inherit">
            {part}
          </mark>
        ) : (
          part
        ),
      )}
    </>
  );
}

/** A distribution of the current results. Click a bar to filter the table to that range. */
function Histogram({
  tab,
  rows,
  all,
  hist,
  onPick,
  title,
}: {
  tab: Tab;
  rows: Row[];
  all: Row[];
  hist: Hist | null;
  onPick: (h: Hist | null) => void;
  title: string;
}) {
  let bars: { key: string; n: number; label: string; pressed: boolean; pick: Hist }[] = [];
  let axis: [string, string, string] = ["", "", ""];
  if (tab === "orgs") {
    const counts: Record<string, number> = {};
    rows.forEach((r) => r.kinds?.forEach((k) => (counts[k] = (counts[k] ?? 0) + 1)));
    const ks = Object.keys(KIND)
      .filter((k) => counts[k])
      .sort((a, b) => counts[b] - counts[a]);
    bars = ks.map((k) => ({ key: k, n: counts[k], label: `${KIND[k]}: ${counts[k]}`, pressed: hist?.cat === k, pick: { cat: k, label: KIND[k] } }));
    axis = [ks[0] ? KIND[ks[0]] : "", `${ks.length} kinds`, ""];
  } else {
    const vals = rows.map((r) => r.lo ?? 0).filter((v) => v > 0);
    const gall = all.map((r) => r.lo ?? 0).filter((v) => v > 0);
    if (vals.length && gall.length) {
      const N = 14;
      const gl = Math.log(Math.min(...gall));
      const gh = Math.log(Math.max(...gall)) + 1e-9;
      const step = (gh - gl) / N;
      const bins = Array.from({ length: N }, (_, i) => ({ a: Math.exp(gl + i * step), b: Math.exp(gl + (i + 1) * step), n: 0 }));
      vals.forEach((v) => {
        bins[Math.min(N - 1, Math.floor((Math.log(v) - gl) / step))].n++;
      });
      bars = bins.map((b, i) => ({
        key: String(i),
        n: b.n,
        label: `${money(b.a)} to ${money(b.b)}: ${b.n}`,
        pressed: hist?.a === b.a,
        pick: { a: b.a, b: b.b, label: `${money(b.a)} to ${money(b.b)}` },
      }));
      axis = [money(bins[0].a), "log scale", money(bins[N - 1].b)];
    }
  }
  const mx = Math.max(...bars.map((b) => b.n), 1);
  return (
    <div className="flex min-w-0 flex-col border-line px-[18px] pt-3.5 pb-2.5 max-lg:border-t lg:border-l">
      <div className="mb-1.5 flex items-baseline justify-between gap-3">
        <b className="text-[14px] font-semibold">{title}</b>
        <span className="text-[13px] text-ink-3">Click a bar to filter</span>
      </div>
      {bars.length === 0 ? (
        <p className="py-6 text-[13px] text-ink-3">No prices to chart.</p>
      ) : (
        <>
          <div role="group" aria-label={title} className="flex min-h-[92px] flex-1 items-end gap-[3px]">
            {bars.map((b) => (
              <button
                key={b.key}
                aria-pressed={b.pressed}
                aria-label={b.label}
                title={b.label}
                onClick={() => onPick(b.pressed ? null : b.pick)}
                className="group/bar relative flex h-[92px] flex-1 flex-col justify-end"
              >
                <i className={`block min-h-0.5 rounded-t-[3px] transition-[height] ${b.pressed ? "bg-s-7" : "bg-line-strong group-hover/bar:bg-ink-2"}`} style={{ height: `${(b.n / mx) * 100}%` }} />
              </button>
            ))}
          </div>
          <div className="mt-1.5 flex justify-between font-mono text-[12px] text-ink-3">
            <span>{axis[0]}</span>
            <span>{axis[1]}</span>
            <span>{axis[2]}</span>
          </div>
        </>
      )}
    </div>
  );
}
