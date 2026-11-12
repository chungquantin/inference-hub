"use client";

import Link from "next/link";
import { useMemo, useState } from "react";
import type { GpuCell, GpuRank, MarketAnalytics, MakerRank, ModelRank, ProviderRank } from "@inference-hub/api-client";
import { HyperLink } from "@/components/hyper-link";
import { Frame } from "@/components/ui/primitives";
import { formatPrice, formatTokens } from "@/lib/format";
import { gpuHref, modelHref } from "@/lib/hrefs";
import { Histogram, IndexBar, LogScatter, RangeBar, SERIES, heat, shortPrice, type ScatterPoint } from "./charts";

type Tab = "models" | "gpus" | "providers";
const TABS: { id: Tab; label: string }[] = [
  { id: "models", label: "Models" },
  { id: "gpus", label: "GPUs" },
  { id: "providers", label: "Providers" },
];

export function RankingsDashboard({ data, initialTab }: { data: MarketAnalytics; initialTab: Tab }) {
  const [tab, setTab] = useState<Tab>(initialTab);
  const choose = (next: Tab) => {
    setTab(next);
    // Keep the tab in the address so a view can be shared, without a navigation.
    const url = new URL(window.location.href);
    url.searchParams.set("tab", next);
    window.history.replaceState(null, "", url);
  };
  return (
    <div className="flex flex-col gap-6">
      <div role="tablist" aria-label="Rankings" className="flex gap-1 border-b border-line">
        {TABS.map((t) => (
          <button
            key={t.id}
            role="tab"
            aria-selected={tab === t.id}
            onClick={() => choose(t.id)}
            className={`-mb-px h-11 border-b-2 px-5 text-[15px] font-medium ${
              tab === t.id ? "border-s-7 text-s-7" : "border-transparent text-ink-2 hover:text-ink"
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>
      {tab === "models" && <ModelsView data={data} />}
      {tab === "gpus" && <GpusView gpus={data.gpus} cells={data.gpu_prices} />}
      {tab === "providers" && <ProvidersView providers={data.providers} />}
    </div>
  );
}

/* ------------------------------------------------------------------ shared */

type Dir = "asc" | "desc";

function useSort<K extends string>(initial: K, dir: Dir = "desc") {
  const [state, setState] = useState<{ key: K; dir: Dir }>({ key: initial, dir });
  const toggle = (key: K, natural: Dir = "desc") =>
    setState((s) => (s.key === key ? { key, dir: s.dir === "asc" ? "desc" : "asc" } : { key, dir: natural }));
  return [state, toggle] as const;
}

function sortRows<T>(rows: T[], value: (row: T) => number | string | null | undefined, dir: Dir): T[] {
  const sign = dir === "asc" ? 1 : -1;
  return [...rows].sort((a, b) => {
    const [x, y] = [value(a), value(b)];
    if (x == null && y == null) return 0;
    if (x == null) return 1; // missing values last, whichever the direction
    if (y == null) return -1;
    return (typeof x === "string" ? x.localeCompare(String(y)) : x - (y as number)) * sign;
  });
}

function Th<K extends string>({
  id,
  label,
  sort,
  onSort,
  natural = "desc",
  numeric = true,
  hint,
}: {
  id: K;
  label: string;
  sort: { key: K; dir: Dir };
  onSort: (key: K, natural?: Dir) => void;
  natural?: Dir;
  numeric?: boolean;
  hint?: string;
}) {
  const on = sort.key === id;
  return (
    <th
      scope="col"
      aria-sort={on ? (sort.dir === "asc" ? "ascending" : "descending") : "none"}
      className={`h-10 px-3 text-[13px] font-medium whitespace-nowrap ${numeric ? "text-right" : "text-left"}`}
    >
      <button
        onClick={() => onSort(id, natural)}
        title={hint}
        className={`inline-flex items-center gap-1 hover:text-ink ${on ? "text-ink" : "text-ink-3"}`}
      >
        {label}
        <span aria-hidden className="w-2 text-[10px]">
          {on ? (sort.dir === "asc" ? "▲" : "▼") : ""}
        </span>
      </button>
    </th>
  );
}

function Panel({ title, note, children, right }: { title: string; note?: string; children: React.ReactNode; right?: React.ReactNode }) {
  return (
    <Frame as="section" className="flex flex-col">
      <header className="flex flex-wrap items-baseline justify-between gap-2 border-b border-line px-4 py-3">
        <div>
          <h2 className="font-display text-[19px] leading-6 font-medium tracking-[-0.03em] text-ink">{title}</h2>
          {note && <p className="max-w-[70ch] text-[13px] leading-5 text-ink-3">{note}</p>}
        </div>
        {right}
      </header>
      {children}
    </Frame>
  );
}

function Kpi({ value, label, sub }: { value: React.ReactNode; label: string; sub?: React.ReactNode }) {
  return (
    <div className="px-5 py-4">
      <span className="block font-display text-[28px] leading-9 font-semibold tracking-[-0.03em] text-ink tabular">{value}</span>
      <span className="block text-[14px] leading-5 text-ink-2">{label}</span>
      {sub && <span className="block truncate text-[13px] leading-5 text-ink-3">{sub}</span>}
    </div>
  );
}

const median = (xs: number[]) => {
  if (xs.length === 0) return 0;
  const s = [...xs].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

const fieldClass =
  "h-9 border border-line-strong bg-wash px-2.5 text-[14px] text-ink outline-none focus:border-s-7 focus-visible:ring-2 focus-visible:ring-s-3";

const CAPS = [
  { id: "tools", label: "Tools" },
  { id: "reasoning", label: "Reasoning" },
  { id: "in:image", label: "Vision" },
  { id: "json", label: "JSON mode" },
];

/* ------------------------------------------------------------------ models */

type ModelSort = "spread" | "min" | "median" | "max" | "providers" | "context" | "name";

function ModelsView({ data }: { data: MarketAnalytics }) {
  const [q, setQ] = useState("");
  const [maker, setMaker] = useState("");
  const [minProviders, setMinProviders] = useState(2);
  const [caps, setCaps] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [limit, setLimit] = useState(25);
  const [sort, toggle] = useSort<ModelSort>("spread");

  const makers = useMemo(() => {
    const counts = new Map<string, number>();
    for (const m of data.models) if (m.maker) counts.set(m.maker, (counts.get(m.maker) ?? 0) + 1);
    return [...counts.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  }, [data.models]);
  const maxProviders = Math.max(2, ...data.models.map((m) => m.providers));

  const filtered = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return data.models.filter(
      (m) =>
        m.providers >= minProviders &&
        (!maker || m.maker === maker) &&
        caps.every((c) => m.capabilities.includes(c)) &&
        (!needle || m.name.toLowerCase().includes(needle) || m.model_key.includes(needle)),
    );
  }, [data.models, q, maker, minProviders, caps]);

  const value = (m: ModelRank): number | string | null =>
    ({
      spread: m.spread,
      min: m.min_blended_per_mtok,
      median: m.median_blended_per_mtok,
      max: m.max_blended_per_mtok,
      providers: m.providers,
      context: m.max_context_window ?? null,
      name: m.name,
    })[sort.key];
  const rows = useMemo(() => sortRows(filtered, value, sort.dir), [filtered, sort]); // eslint-disable-line react-hooks/exhaustive-deps

  const makerColor = useMemo(() => {
    const top = makers.slice(0, SERIES.length - 1).map(([k]) => k);
    return (m: string | null) => {
      const i = m ? top.indexOf(m) : -1;
      return i >= 0 ? SERIES[i] : "#9a968c";
    };
  }, [makers]);

  const points: ScatterPoint[] = filtered.map((m) => ({
    id: m.model_key,
    x: m.min_blended_per_mtok,
    y: Math.max(1, m.spread),
    r: 3 + Math.sqrt(m.providers) * 1.6,
    color: makerColor(m.maker ?? null),
    label: m.name,
    detail: `${shortPrice(m.min_blended_per_mtok)} cheapest · ${m.spread.toFixed(1)}× spread · ${m.providers} providers`,
  }));

  const spreads = filtered.map((m) => m.spread);
  const biggest = [...filtered].sort((a, b) => b.spread - a.spread)[0];
  const cheapestWide = [...filtered].filter((m) => m.providers >= 5).sort((a, b) => a.min_blended_per_mtok - b.min_blended_per_mtok)[0];
  const domain: [number, number] = [
    Math.min(0.001, ...rows.map((m) => m.min_blended_per_mtok)) || 0.001,
    Math.max(10, ...rows.map((m) => m.max_blended_per_mtok)),
  ];
  const sel = data.models.find((m) => m.model_key === selected);

  return (
    <div className="flex flex-col gap-6">
      <Frame className="grid grid-cols-2 sm:grid-cols-4">
        <Kpi value={filtered.length.toLocaleString()} label="contested models" sub={`sold by ${minProviders}+ providers`} />
        <Kpi value={`${median(spreads).toFixed(1)}×`} label="median spread" sub="priciest provider ÷ cheapest" />
        <Kpi
          value={biggest ? `${biggest.spread.toFixed(0)}×` : "—"}
          label="biggest spread"
          sub={biggest ? biggest.name.split("/").pop() : undefined}
        />
        <Kpi
          value={cheapestWide ? shortPrice(cheapestWide.min_blended_per_mtok) : "—"}
          label="cheapest on 5+ providers"
          sub={cheapestWide ? cheapestWide.name.split("/").pop() : undefined}
        />
      </Frame>

      <Frame className="flex flex-wrap items-end gap-4 p-4">
        <label className="flex min-w-[200px] flex-1 flex-col gap-1 text-[13px] text-ink-3">
          Search
          <input className={fieldClass} value={q} onChange={(e) => setQ(e.target.value)} placeholder="deepseek, llama 70b…" />
        </label>
        <label className="flex flex-col gap-1 text-[13px] text-ink-3">
          Maker
          <select className={fieldClass} value={maker} onChange={(e) => setMaker(e.target.value)}>
            <option value="">All makers</option>
            {makers.map(([m, n]) => (
              <option key={m} value={m}>
                {m} ({n})
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-[13px] text-ink-3">
          Providers ≥ {minProviders}
          <input
            type="range"
            min={2}
            max={maxProviders}
            value={minProviders}
            onChange={(e) => setMinProviders(Number(e.target.value))}
            className="h-9 w-36 accent-[var(--s-7)]"
          />
        </label>
        <fieldset className="flex flex-col gap-1 text-[13px] text-ink-3">
          <legend className="mb-1">Can do</legend>
          <span className="flex gap-1">
            {CAPS.map((c) => {
              const on = caps.includes(c.id);
              return (
                <button
                  key={c.id}
                  aria-pressed={on}
                  onClick={() => setCaps(on ? caps.filter((x) => x !== c.id) : [...caps, c.id])}
                  className={`h-9 px-3 text-[14px] ${
                    on ? "bg-s-7 text-wash" : "bg-wash text-ink shadow-[inset_0_0_0_1px_var(--line-strong)] hover:bg-s-1"
                  }`}
                >
                  {c.label}
                </button>
              );
            })}
          </span>
        </fieldset>
      </Frame>

      <div className="grid gap-6 lg:grid-cols-[1.35fr_1fr]">
        <Panel
          title="Where shopping around pays"
          note="Each dot is a model. Right means pricier at its cheapest provider; up means a wider gap between the cheapest and priciest provider. Bigger dots are sold by more providers."
        >
          <div className="p-3">
            <LogScatter
              points={points}
              xLabel="Cheapest price · USD per 1M tokens (3:1 blend)"
              yLabel="Spread · priciest ÷ cheapest"
              selected={selected}
              onSelect={setSelected}
            />
            <ul className="flex flex-wrap gap-x-4 gap-y-1 px-3 pb-2 text-[12px] text-ink-2">
              {makers.slice(0, SERIES.length - 1).map(([m]) => (
                <li key={m} className="flex items-center gap-1.5">
                  <span className="inline-block size-2.5" style={{ background: makerColor(m) }} />
                  {m}
                </li>
              ))}
              <li className="flex items-center gap-1.5">
                <span className="inline-block size-2.5" style={{ background: "#9a968c" }} />
                other
              </li>
            </ul>
          </div>
          {sel && <ModelDetail model={sel} onClose={() => setSelected(null)} />}
        </Panel>

        <div className="flex flex-col gap-6">
          <Panel title="Price distribution" note="Chat models by their cheapest price per million tokens. Dark: sold by 2+ providers.">
            <div className="p-3">
              <Histogram buckets={data.price_histogram} />
            </div>
          </Panel>
          <MakersTable makers={data.makers} />
        </div>
      </div>

      <Panel
        title={`Ranking · ${rows.length.toLocaleString()} models`}
        note="Click a column to sort. The bar spans cheapest to priciest provider on a log scale; green marks the cheapest, blue the median."
      >
        <div className="overflow-x-auto">
          <table className="w-full border-collapse text-[14px] leading-5">
            <thead>
              <tr className="border-b border-line bg-paper">
                <th className="h-10 w-10 px-3 text-right text-[13px] font-medium text-ink-3">#</th>
                <Th id="name" label="Model" sort={sort} onSort={toggle} natural="asc" numeric={false} />
                <Th id="providers" label="Providers" sort={sort} onSort={toggle} />
                <Th id="min" label="Cheapest" sort={sort} onSort={toggle} natural="asc" />
                <Th id="median" label="Median" sort={sort} onSort={toggle} natural="asc" />
                <Th id="max" label="Priciest" sort={sort} onSort={toggle} natural="asc" />
                <th className="h-10 px-3 text-left text-[13px] font-medium text-ink-3">Range</th>
                <Th id="spread" label="Spread" sort={sort} onSort={toggle} hint="Priciest provider ÷ cheapest" />
                <Th id="context" label="Context" sort={sort} onSort={toggle} />
                <th className="h-10 px-3 text-left text-[13px] font-medium text-ink-3">Cheapest at</th>
              </tr>
            </thead>
            <tbody>
              {rows.slice(0, limit).map((m, i) => (
                <tr
                  key={m.model_key}
                  className={`border-b border-line last:border-0 ${selected === m.model_key ? "bg-s-1" : "hover:bg-paper"}`}
                >
                  <td className="h-12 px-3 text-right font-mono text-[12px] text-ink-3 tabular">{i + 1}</td>
                  <td className="max-w-[260px] px-3">
                    <button onClick={() => setSelected(m.model_key)} className="flex flex-col text-left">
                      <span className="truncate font-medium text-ink hover:text-s-7">{m.name.split("/").pop()}</span>
                      <span className="truncate text-[12px] text-ink-3">{m.maker}</span>
                    </button>
                  </td>
                  <td className="px-3 text-right font-mono text-[13px] tabular">{m.providers}</td>
                  <td className="px-3 text-right font-mono text-[13px] font-medium text-lo tabular">{shortPrice(m.min_blended_per_mtok)}</td>
                  <td className="px-3 text-right font-mono text-[13px] text-ink-2 tabular">{shortPrice(m.median_blended_per_mtok)}</td>
                  <td className="px-3 text-right font-mono text-[13px] text-ink-2 tabular">{shortPrice(m.max_blended_per_mtok)}</td>
                  <td className="w-[180px] px-3">
                    <RangeBar
                      min={m.min_blended_per_mtok}
                      median={m.median_blended_per_mtok}
                      max={m.max_blended_per_mtok}
                      domain={domain}
                      label={`${shortPrice(m.min_blended_per_mtok)} to ${shortPrice(m.max_blended_per_mtok)}`}
                    />
                  </td>
                  <td className="px-3 text-right font-mono text-[13px] font-medium tabular">{m.spread.toFixed(1)}×</td>
                  <td className="px-3 text-right font-mono text-[13px] text-ink-2 tabular">{formatTokens(m.max_context_window)}</td>
                  <td className="px-3">
                    <HyperLink type="org" slug={m.cheapest.slug} name={m.cheapest.name} logoUrl={m.cheapest.logo_url} />
                  </td>
                </tr>
              ))}
              {rows.length === 0 && (
                <tr>
                  <td colSpan={10} className="h-24 text-center text-ink-3">
                    No models match these filters.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
        {rows.length > limit && (
          <button onClick={() => setLimit(limit + 50)} className="h-11 border-t border-line text-[14px] font-medium text-s-7 hover:bg-s-1">
            Show 50 more · {rows.length - limit} left
          </button>
        )}
      </Panel>
    </div>
  );
}

function ModelDetail({ model, onClose }: { model: ModelRank; onClose: () => void }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-4 border-t border-line bg-s-1 px-4 py-3">
      <div className="min-w-0">
        <p className="truncate font-medium text-ink">{model.name}</p>
        <p className="font-mono text-[12px] text-ink-2 tabular">
          {shortPrice(model.min_blended_per_mtok)} cheapest · {shortPrice(model.median_blended_per_mtok)} median ·{" "}
          {shortPrice(model.max_blended_per_mtok)} priciest · in {formatPrice(model.min_input_per_mtok)} / out{" "}
          {formatPrice(model.min_output_per_mtok)} · {model.providers} providers
        </p>
      </div>
      <span className="flex gap-3 text-[14px]">
        <Link href={modelHref(model.model_key)} className="font-medium text-s-7 hover:underline">
          All prices
        </Link>
        <button onClick={onClose} className="text-ink-3 hover:text-ink">
          Clear
        </button>
      </span>
    </div>
  );
}

function MakersTable({ makers }: { makers: MakerRank[] }) {
  const max = Math.max(...makers.map((m) => m.contested_models), 1);
  return (
    <Panel title="Makers" note="Models sold by 2+ providers, and the typical cheapest price of the maker's whole catalog.">
      <div className="max-h-[360px] overflow-auto">
        <table className="w-full border-collapse text-[13px]">
          <thead className="sticky top-0 bg-paper text-ink-3">
            <tr className="border-b border-line">
              <th className="h-9 px-3 text-left font-medium">Maker</th>
              <th className="h-9 px-3 text-right font-medium">Contested</th>
              <th className="h-9 w-24 px-3" />
              <th className="h-9 px-3 text-right font-medium">Median low</th>
            </tr>
          </thead>
          <tbody>
            {makers.map((m) => (
              <tr key={m.maker} className="border-b border-line last:border-0 hover:bg-paper">
                <td className="h-10 px-3">
                  <HyperLink type="creator" maker={m.maker} />
                </td>
                <td className="px-3 text-right font-mono tabular" title={`${m.models} priced models, ${(m.contested_share * 100).toFixed(0)}% contested`}>
                  {m.contested_models}
                  <span className="text-ink-3"> / {m.models}</span>
                </td>
                <td className="px-3">
                  <span role="img" aria-label={`${m.contested_models} contested models`} className="block h-2 bg-field">
                    <span className="block h-full bg-s-5" style={{ width: `${(m.contested_models / max) * 100}%` }} />
                  </span>
                </td>
                <td className="px-3 text-right font-mono text-ink-2 tabular">{shortPrice(m.median_min_price)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Panel>
  );
}

/* -------------------------------------------------------------------- GPUs */

type GpuSort = "name" | "providers" | "min" | "median" | "max" | "spread";

function GpusView({ gpus, cells }: { gpus: GpuRank[]; cells: GpuCell[] }) {
  const [minProviders, setMinProviders] = useState(2);
  const [sort, toggle] = useSort<GpuSort>("providers");
  const filtered = gpus.filter((g) => g.providers >= minProviders);
  const value = (g: GpuRank): number | string =>
    ({ name: g.gpu_type, providers: g.providers, min: g.min_per_gpu_hour, median: g.median_per_gpu_hour, max: g.max_per_gpu_hour, spread: g.spread })[sort.key];
  const rows = sortRows(filtered, value, sort.dir);
  const domain: [number, number] = [Math.min(...gpus.map((g) => g.min_per_gpu_hour), 0.1), Math.max(...gpus.map((g) => g.max_per_gpu_hour), 1)];
  const widest = [...filtered].sort((a, b) => b.spread - a.spread)[0];
  const cheapestH100 = gpus.find((g) => g.gpu_type === "H100 SXM");

  return (
    <div className="flex flex-col gap-6">
      <Frame className="grid grid-cols-2 sm:grid-cols-4">
        <Kpi value={filtered.length} label="GPU types" sub={`sold by ${minProviders}+ clouds`} />
        <Kpi value={`${median(filtered.map((g) => g.spread)).toFixed(1)}×`} label="median spread" sub="priciest listing ÷ cheapest" />
        <Kpi value={widest ? `${widest.spread.toFixed(0)}×` : "—"} label="widest spread" sub={widest?.gpu_type} />
        <Kpi
          value={cheapestH100 ? formatPrice(cheapestH100.min_per_gpu_hour) : "—"}
          label="cheapest H100 SXM / hr"
          sub={cheapestH100 ? `at ${cheapestH100.cheapest.name}` : undefined}
        />
      </Frame>

      <HeatMatrix cells={cells} gpus={gpus} />

      <Panel
        title={`Ranking · ${rows.length} GPU types`}
        note="Price per GPU-hour across every listing, on demand and marketplace. The bar spans cheapest to priciest on a log scale."
        right={
          <label className="flex items-center gap-2 text-[13px] text-ink-3">
            Clouds ≥ {minProviders}
            <input
              type="range"
              min={1}
              max={Math.max(...gpus.map((g) => g.providers))}
              value={minProviders}
              onChange={(e) => setMinProviders(Number(e.target.value))}
              className="w-28 accent-[var(--s-7)]"
            />
          </label>
        }
      >
        <div className="overflow-x-auto">
          <table className="w-full border-collapse text-[14px] leading-5">
            <thead>
              <tr className="border-b border-line bg-paper">
                <th className="h-10 w-10 px-3 text-right text-[13px] font-medium text-ink-3">#</th>
                <Th id="name" label="GPU" sort={sort} onSort={toggle} natural="asc" numeric={false} />
                <Th id="providers" label="Clouds" sort={sort} onSort={toggle} />
                <Th id="min" label="Cheapest" sort={sort} onSort={toggle} natural="asc" />
                <Th id="median" label="Median" sort={sort} onSort={toggle} natural="asc" />
                <Th id="max" label="Priciest" sort={sort} onSort={toggle} natural="asc" />
                <th className="h-10 px-3 text-left text-[13px] font-medium text-ink-3">Range</th>
                <Th id="spread" label="Spread" sort={sort} onSort={toggle} />
                <th className="h-10 px-3 text-left text-[13px] font-medium text-ink-3">Cheapest at</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((g, i) => (
                <tr key={g.gpu_type} className="border-b border-line last:border-0 hover:bg-paper">
                  <td className="h-12 px-3 text-right font-mono text-[12px] text-ink-3 tabular">{i + 1}</td>
                  <td className="px-3">
                    <HyperLink type="gpu" gpuType={g.gpu_type} href={gpuHref(g.gpu_type)} />
                  </td>
                  <td className="px-3 text-right font-mono text-[13px] tabular">{g.providers}</td>
                  <td className="px-3 text-right font-mono text-[13px] font-medium text-lo tabular">{formatPrice(g.min_per_gpu_hour)}</td>
                  <td className="px-3 text-right font-mono text-[13px] text-ink-2 tabular">{formatPrice(g.median_per_gpu_hour)}</td>
                  <td className="px-3 text-right font-mono text-[13px] text-ink-2 tabular">{formatPrice(g.max_per_gpu_hour)}</td>
                  <td className="w-[200px] px-3">
                    <RangeBar
                      min={g.min_per_gpu_hour}
                      median={g.median_per_gpu_hour}
                      max={g.max_per_gpu_hour}
                      domain={domain}
                      label={`${formatPrice(g.min_per_gpu_hour)} to ${formatPrice(g.max_per_gpu_hour)} per GPU-hour`}
                    />
                  </td>
                  <td className="px-3 text-right font-mono text-[13px] font-medium tabular">{g.spread.toFixed(1)}×</td>
                  <td className="px-3">
                    <HyperLink type="org" slug={g.cheapest.slug} name={g.cheapest.name} logoUrl={g.cheapest.logo_url} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>
    </div>
  );
}

/** GPU types by clouds: each cell is the cloud's lowest price, colored against the market median. */
function HeatMatrix({ cells, gpus }: { cells: GpuCell[]; gpus: GpuRank[] }) {
  const [rowsShown, setRowsShown] = useState(14);
  const byGpu = useMemo(() => {
    const map = new Map<string, Map<string, GpuCell>>();
    for (const c of cells) {
      const row = map.get(c.gpu_type) ?? new Map<string, GpuCell>();
      row.set(c.provider.slug, c);
      map.set(c.gpu_type, row);
    }
    return map;
  }, [cells]);
  const gpuRows = gpus.filter((g) => byGpu.has(g.gpu_type)).slice(0, rowsShown);
  const cols = useMemo(() => {
    const count = new Map<string, { name: string; logo: string | null | undefined; n: number }>();
    for (const g of gpuRows)
      for (const c of byGpu.get(g.gpu_type)?.values() ?? []) {
        const e = count.get(c.provider.slug) ?? { name: c.provider.name, logo: c.provider.logo_url, n: 0 };
        e.n++;
        count.set(c.provider.slug, e);
      }
    return [...count.entries()].sort((a, b) => b[1].n - a[1].n || a[1].name.localeCompare(b[1].name));
  }, [gpuRows, byGpu]);

  return (
    <Panel
      title="Price matrix"
      note="Each cell is that cloud's lowest price per GPU-hour. Green is cheaper than the market median, red pricier; the outlined cell is the cheapest for that GPU."
    >
      <div className="overflow-x-auto p-3">
        <table className="border-separate border-spacing-[2px] text-[12px]">
          <thead>
            <tr>
              <th className="sticky left-0 z-10 bg-wash" />
              {cols.map(([slug, c]) => (
                <th key={slug} className="h-24 w-14 align-bottom font-normal">
                  <Link href={`/organizations/${slug}`} title={c.name} className="mx-auto block w-14 origin-bottom-left text-ink-2 hover:text-s-7">
                    <span className="block h-20 [writing-mode:vertical-rl] rotate-180 truncate text-left">{c.name}</span>
                  </Link>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {gpuRows.map((g) => (
              <tr key={g.gpu_type}>
                <th scope="row" className="sticky left-0 z-10 bg-wash pr-3 text-left font-medium whitespace-nowrap">
                  <Link href={gpuHref(g.gpu_type)} className="text-ink hover:text-s-7">
                    {g.gpu_type}
                  </Link>
                </th>
                {cols.map(([slug]) => {
                  const c = byGpu.get(g.gpu_type)?.get(slug);
                  if (!c) return <td key={slug} className="h-8 bg-field/60" />;
                  const h = heat(c.vs_median);
                  return (
                    <td
                      key={slug}
                      title={`${c.provider.name} · ${g.gpu_type}: ${formatPrice(c.per_gpu_hour)}/GPU-hr · ${c.vs_median.toFixed(2)}× the median${c.rank === 1 ? " · cheapest" : ""}`}
                      className={`h-8 min-w-14 text-center font-mono tabular ${c.rank === 1 ? "outline-2 -outline-offset-2 outline-ink" : ""}`}
                      style={{ background: h.bg, color: h.fg }}
                    >
                      {c.per_gpu_hour < 10 ? c.per_gpu_hour.toFixed(2) : c.per_gpu_hour.toFixed(1)}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {gpus.filter((g) => byGpu.has(g.gpu_type)).length > rowsShown && (
        <button onClick={() => setRowsShown(rowsShown + 20)} className="h-11 border-t border-line text-[14px] font-medium text-s-7 hover:bg-s-1">
          Show more GPU types
        </button>
      )}
    </Panel>
  );
}

/* --------------------------------------------------------------- providers */

type ProviderSort = "name" | "wins" | "model_index" | "gpu_index" | "contested" | "catalog" | "win_rate";

function ProvidersView({ providers }: { providers: ProviderRank[] }) {
  const [sort, toggle] = useSort<ProviderSort>("wins");
  const [selected, setSelected] = useState<string | null>(null);
  const contested = (p: ProviderRank) => p.contested_models + p.contested_gpus;
  const wins = (p: ProviderRank) => p.model_wins + p.gpu_wins;
  const winRate = (p: ProviderRank) => (contested(p) ? wins(p) / contested(p) : null);
  const value = (p: ProviderRank): number | string | null | undefined =>
    ({
      name: p.organization.name,
      wins: wins(p),
      model_index: p.model_price_index,
      gpu_index: p.gpu_price_index,
      contested: contested(p),
      catalog: p.models + p.media_models + p.gpu_configurations,
      win_rate: winRate(p),
    })[sort.key];
  const rows = sortRows(
    providers.filter((p) => contested(p) > 0 || p.models + p.gpu_configurations > 0),
    value,
    sort.dir,
  );
  const scored = providers.filter((p) => p.model_price_index != null && p.contested_models >= 3);
  const points: ScatterPoint[] = scored.map((p, i) => ({
    id: p.organization.slug,
    x: p.contested_models,
    y: p.model_price_index as number,
    r: 4 + Math.sqrt(p.model_wins) * 1.3,
    color: SERIES[i % SERIES.length],
    label: p.organization.name,
    detail: `${p.contested_models} contested models · ${(p.model_price_index as number).toFixed(2)}× median · ${p.model_wins} cheapest`,
  }));
  const leader = [...scored].sort((a, b) => (a.model_price_index as number) - (b.model_price_index as number))[0];
  const topWins = [...providers].sort((a, b) => wins(b) - wins(a))[0];
  const gpuLeader = [...providers].filter((p) => p.gpu_price_index != null && p.contested_gpus >= 3).sort((a, b) => (a.gpu_price_index as number) - (b.gpu_price_index as number))[0];

  return (
    <div className="flex flex-col gap-6">
      <Frame className="grid grid-cols-2 sm:grid-cols-4">
        <Kpi value={providers.length} label="providers scored" />
        <Kpi value={topWins ? wins(topWins) : "—"} label="most cheapest-in-market prices" sub={topWins?.organization.name} />
        <Kpi
          value={leader ? `${(leader.model_price_index as number).toFixed(2)}×` : "—"}
          label="lowest model price index"
          sub={leader?.organization.name}
        />
        <Kpi
          value={gpuLeader ? `${(gpuLeader.gpu_price_index as number).toFixed(2)}×` : "—"}
          label="lowest GPU price index"
          sub={gpuLeader?.organization.name}
        />
      </Frame>

      <Panel
        title="Breadth against price"
        note="Providers with 3+ contested models. Further right means more models also sold elsewhere; lower means cheaper than the market median on the models they share. Bigger dots hold more cheapest prices."
      >
        <div className="mx-auto w-full max-w-[900px] p-3">
          <LogScatter
            labelFrom={10}
            points={points}
            xLabel="Contested chat models sold"
            yLabel="Price index · 1× is the market median"
            xFormat={(v) => String(Math.round(v))}
            yFormat={(v) => `${v.toFixed(v < 1 ? 2 : 1)}×`}
            selected={selected}
            onSelect={setSelected}
          />
        </div>
      </Panel>

      <Panel
        title="Scoreboard"
        note="Price index is the geometric mean of a provider's price over the market median, across items at least one other provider also sells: 0.50× is half the median, 2× is double. Wins count items where it is the cheapest."
      >
        <div className="overflow-x-auto">
          <table className="w-full border-collapse text-[14px] leading-5">
            <thead>
              <tr className="border-b border-line bg-paper">
                <th className="h-10 w-10 px-3 text-right text-[13px] font-medium text-ink-3">#</th>
                <Th id="name" label="Provider" sort={sort} onSort={toggle} natural="asc" numeric={false} />
                <Th id="wins" label="Cheapest" sort={sort} onSort={toggle} hint="Models and GPU types where it has the lowest price" />
                <Th id="win_rate" label="Win rate" sort={sort} onSort={toggle} hint="Cheapest ÷ contested items" />
                <Th id="model_index" label="Model price index" sort={sort} onSort={toggle} natural="asc" numeric={false} />
                <Th id="gpu_index" label="GPU price index" sort={sort} onSort={toggle} natural="asc" numeric={false} />
                <Th id="contested" label="Contested" sort={sort} onSort={toggle} hint="Models + GPU types shared with another provider" />
                <Th id="catalog" label="Catalog" sort={sort} onSort={toggle} hint="Chat + media models + GPU configurations" />
              </tr>
            </thead>
            <tbody>
              {rows.map((p, i) => (
                <tr
                  key={p.organization.slug}
                  className={`border-b border-line last:border-0 ${selected === p.organization.slug ? "bg-s-1" : "hover:bg-paper"}`}
                >
                  <td className="h-12 px-3 text-right font-mono text-[12px] text-ink-3 tabular">{i + 1}</td>
                  <td className="px-3">
                    <HyperLink type="org" slug={p.organization.slug} name={p.organization.name} logoUrl={p.organization.logo_url} />
                  </td>
                  <td className="px-3 text-right font-mono text-[13px] tabular">
                    <span className="font-medium text-ink">{wins(p)}</span>
                    <span className="text-ink-3"> ({p.model_wins}m · {p.gpu_wins}g)</span>
                  </td>
                  <td className="px-3 text-right font-mono text-[13px] text-ink-2 tabular">
                    {winRate(p) == null ? "—" : `${Math.round((winRate(p) as number) * 100)}%`}
                  </td>
                  <td className="w-[200px] px-3">
                    <IndexCell value={p.model_price_index} />
                  </td>
                  <td className="w-[200px] px-3">
                    <IndexCell value={p.gpu_price_index} />
                  </td>
                  <td className="px-3 text-right font-mono text-[13px] text-ink-2 tabular">
                    {contested(p)}
                    <span className="text-ink-3"> ({p.contested_models}m · {p.contested_gpus}g)</span>
                  </td>
                  <td className="px-3 text-right font-mono text-[13px] text-ink-2 tabular">
                    {(p.models + p.media_models + p.gpu_configurations).toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>
    </div>
  );
}

function IndexCell({ value }: { value: number | null | undefined }) {
  if (value == null) return <span className="text-ink-3">—</span>;
  return (
    <span className="flex items-center gap-2">
      <span className="w-12 text-right font-mono text-[13px] tabular">{value.toFixed(2)}×</span>
      <IndexBar value={value} label={`${value.toFixed(2)} times the market median`} />
    </span>
  );
}
