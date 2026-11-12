"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import type { CurveItem, GpuCurve, ModelCurve, SnapshotPoint, TermPoint } from "@inference-hub/api-client";
import { SERIES, shortPrice } from "@/components/rankings/charts";
import { Logo } from "@/components/ui/logo";
import { formatPrice } from "@/lib/format";

/** Spot, on demand, then committed terms, left to right: the order of flexibility. */
const rank = (p: { tier: string; term_months: number }) => (p.tier === "spot" ? -1 : p.term_months);

function columnLabel(c: { tier: string; term_months: number }): string {
  if (c.tier === "spot") return "Spot";
  if (c.tier === "on_demand") return "On demand";
  const m = c.term_months;
  return m % 12 === 0 ? `${m / 12} ${m === 12 ? "year" : "years"}` : `${m} months`;
}

/** A select that moves to the chosen item by changing the address, so a view can be shared. */
export function Picker({
  label,
  param,
  items,
  value,
  extra,
}: {
  label: string;
  param: string;
  items: CurveItem[];
  value: string;
  extra?: Record<string, string>;
}) {
  const router = useRouter();
  return (
    <label className="flex flex-col gap-1 text-[13px] text-ink-3">
      {label}
      <select
        value={value}
        onChange={(e) => {
          const q = new URLSearchParams({ ...extra, [param]: e.target.value });
          router.replace(`/analytics/curves?${q}`, { scroll: false });
        }}
        className="h-10 min-w-[240px] border border-line-strong bg-wash px-3 text-[15px] text-ink"
      >
        {items.map((i) => (
          <option key={i.key} value={i.key}>
            {i.name} ({i.providers} {i.providers === 1 ? "seller" : "sellers"}
            {i.tiers > 1 ? `, ${i.tiers} price tiers` : ""})
          </option>
        ))}
      </select>
    </label>
  );
}

const W = 760;
const H = 330;
const pad = { top: 16, right: 24, bottom: 36, left: 56 };
const r2 = (n: number) => Math.round(n * 100) / 100;

type Column = { tier: string; term_months: number; key: string };

/**
 * What one GPU-hour costs by how it is bought, left to right from the most flexible (spot)
 * to the longest commitment. Each provider is a line through the tiers it reports; a
 * provider with a single tier is a dot. The grey bar is the market's spread at that tier.
 */
export function TermChart({ curve }: { curve: GpuCurve }) {
  const [hover, setHover] = useState<string | null>(null);
  const columns: Column[] = [
    ...new Map(curve.points.map((p) => [`${p.tier}:${p.term_months}`, { tier: p.tier, term_months: p.term_months, key: `${p.tier}:${p.term_months}` }])).values(),
  ].sort((a, b) => rank(a) - rank(b));
  if (columns.length === 0) return <p className="p-6 text-[14px] text-ink-3">No provider reports a price for this GPU.</p>;

  const byOrg = new Map<string, { name: string; logo?: string | null; points: TermPoint[] }>();
  for (const p of curve.points) {
    const e = byOrg.get(p.organization.slug) ?? { name: p.organization.name, logo: p.organization.logo_url, points: [] };
    e.points.push(p);
    byOrg.set(p.organization.slug, e);
  }
  // Providers that show a curve (2+ tiers) first, then the rest by their cheapest price.
  const orgs = [...byOrg.entries()].sort(
    (a, b) => b[1].points.length - a[1].points.length || Math.min(...a[1].points.map((p) => p.per_gpu_hour)) - Math.min(...b[1].points.map((p) => p.per_gpu_hour)),
  );
  const max = Math.max(...curve.points.map((p) => p.per_gpu_hour));
  const top = niceTop(max);
  const step = (W - pad.left - pad.right) / columns.length;
  const cx = (c: { tier: string; term_months: number }) =>
    r2(pad.left + step * (columns.findIndex((k) => k.tier === c.tier && k.term_months === c.term_months) + 0.5));
  const sy = (v: number) => r2(H - pad.bottom - (v / top) * (H - pad.top - pad.bottom));
  const ticks = Array.from({ length: 5 }, (_, i) => (top / 4) * i);
  // Providers with a curve get their own colour; past the palette the rest share a grey, so no two colours repeat.
  const color = (slug: string) => SERIES[orgs.findIndex(([s]) => s === slug)] ?? "#9a968c";

  return (
    <div className="grid gap-0 lg:grid-cols-[1fr_280px]">
      <div className="min-w-0 border-line p-4 lg:border-r">
        <svg viewBox={`0 0 ${W} ${H}`} className="h-auto w-full" role="img" aria-label={`${curve.gpu_type} price per GPU-hour by term`}>
          {ticks.map((v) => (
            <g key={v}>
              <line x1={pad.left} x2={W - pad.right} y1={sy(v)} y2={sy(v)} stroke="var(--line)" />
              <text x={pad.left - 8} y={sy(v) + 4} textAnchor="end" className="fill-ink-3 font-mono text-[11px]">
                {v === 0 ? "$0" : shortPrice(v)}
              </text>
            </g>
          ))}
          {columns.map((c) => {
            const node = curve.nodes.find((n) => n.tier === c.tier && n.term_months === c.term_months);
            return (
              <g key={c.key}>
                <text x={cx(c)} y={H - 12} textAnchor="middle" className="fill-ink-2 text-[12px]">
                  {columnLabel(c)}
                </text>
                {node && node.providers > 1 && (
                  <rect x={cx(c) - 9} y={sy(node.max)} width={18} height={Math.max(sy(node.min) - sy(node.max), 1)} fill="var(--field)" />
                )}
                {node && (
                  <line x1={cx(c) - 12} x2={cx(c) + 12} y1={sy(node.median)} y2={sy(node.median)} stroke="var(--ink-3)" strokeWidth={1.5} />
                )}
              </g>
            );
          })}
          {orgs.map(([slug, o]) => {
            const pts = [...o.points].sort((a, b) => rank(a) - rank(b));
            const dim = hover && hover !== slug;
            return (
              <g key={slug} opacity={dim ? 0.15 : 1}>
                {pts.length > 1 && (
                  <polyline
                    points={pts.map((p) => `${cx(p)},${sy(p.per_gpu_hour)}`).join(" ")}
                    fill="none"
                    stroke={color(slug)}
                    strokeWidth={hover === slug ? 3 : 2}
                    strokeLinejoin="round"
                  />
                )}
                {pts.map((p) => (
                  <circle key={`${p.tier}${p.term_months}`} cx={cx(p)} cy={sy(p.per_gpu_hour)} r={hover === slug ? 5 : 4} fill={color(slug)} stroke="var(--wash)" strokeWidth={1}>
                    <title>{`${o.name}, ${columnLabel(p)}: ${formatPrice(p.per_gpu_hour)} per GPU-hour`}</title>
                  </circle>
                ))}
              </g>
            );
          })}
        </svg>
        <p className="mt-1 max-w-[70ch] text-[13px] leading-5 text-ink-3">
          Grey bar: lowest to highest price across the providers that report the tier; tick: their median. Spot can be
          interrupted. A committed price is for a fixed term, paid whether or not the GPU is used. Only prices a provider
          publishes are shown.
        </p>
      </div>
      <ul className="flex max-h-[420px] flex-col overflow-y-auto py-1">
        {orgs.map(([slug, o]) => {
          const base = o.points.find((p) => p.tier === "on_demand")?.per_gpu_hour;
          const longest = [...o.points].filter((p) => p.tier === "committed").sort((a, b) => b.term_months - a.term_months)[0];
          return (
            <li
              key={slug}
              onMouseEnter={() => setHover(slug)}
              onMouseLeave={() => setHover(null)}
              className="flex items-center gap-2.5 px-4 py-1.5 text-[14px] leading-5"
            >
              <span className="size-2.5 shrink-0" style={{ background: color(slug) }} aria-hidden />
              <Logo name={o.name} src={o.logo} size="xs" />
              <Link href={`/organizations/${slug}`} className="min-w-0 flex-1 truncate text-ink hover:text-s-7">
                {o.name}
              </Link>
              <span className="font-mono text-[12px] text-ink-3 tabular">
                {longest && base ? `${Math.round((1 - longest.per_gpu_hour / base) * 100)}% off at ${columnLabel(longest).toLowerCase()}` : `${o.points.length} tier${o.points.length === 1 ? "" : "s"}`}
              </span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function niceTop(v: number): number {
  const mag = 10 ** Math.floor(Math.log10(v));
  return ([1, 2, 4, 5, 8, 10].find((m) => m * mag >= v * 1.05) ?? 10) * mag;
}

/** The same numbers as the chart, as a table: one row per provider, one column per tier. */
export function TermTable({ curve }: { curve: GpuCurve }) {
  const columns = [...new Map(curve.points.map((p) => [`${p.tier}:${p.term_months}`, p])).values()].sort((a, b) => rank(a) - rank(b));
  const orgs = new Map<string, { name: string; logo?: string | null; cells: Map<string, TermPoint> }>();
  for (const p of curve.points) {
    const e = orgs.get(p.organization.slug) ?? { name: p.organization.name, logo: p.organization.logo_url, cells: new Map() };
    e.cells.set(`${p.tier}:${p.term_months}`, p);
    orgs.set(p.organization.slug, e);
  }
  const rows = [...orgs.entries()].sort((a, b) => b[1].cells.size - a[1].cells.size || a[1].name.localeCompare(b[1].name));
  return (
    <div className="overflow-x-auto">
      <table className="w-full border-collapse text-[14px] leading-5">
        <thead>
          <tr className="border-b border-line bg-paper">
            <th scope="col" className="h-10 px-4 text-left text-[13px] font-medium text-ink-3">
              Provider
            </th>
            {columns.map((c) => (
              <th key={`${c.tier}${c.term_months}`} scope="col" className="h-10 px-3 text-right text-[13px] font-medium whitespace-nowrap text-ink-3">
                {columnLabel(c)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map(([slug, o]) => {
            const base = o.cells.get("on_demand:0")?.per_gpu_hour;
            return (
              <tr key={slug} className="border-b border-line last:border-0 hover:bg-paper">
                <td className="px-4 py-2">
                  <span className="flex items-center gap-2.5">
                    <Logo name={o.name} src={o.logo} size="xs" />
                    <Link href={`/organizations/${slug}`} className="font-medium text-ink hover:text-s-7">
                      {o.name}
                    </Link>
                  </span>
                </td>
                {columns.map((c) => {
                  const cell = o.cells.get(`${c.tier}:${c.term_months}`);
                  return (
                    <td key={`${c.tier}${c.term_months}`} className="px-3 py-2 text-right font-mono tabular">
                      {cell ? (
                        <>
                          <span className="text-ink">{formatPrice(cell.per_gpu_hour)}</span>
                          {base && c.tier !== "on_demand" && (
                            <span className={`block text-[12px] ${cell.per_gpu_hour < base ? "text-lo" : "text-hi"}`}>
                              {cell.per_gpu_hour < base ? "−" : "+"}
                              {Math.abs(Math.round((1 - cell.per_gpu_hour / base) * 100))}%
                            </span>
                          )}
                        </>
                      ) : (
                        <span className="text-ink-3">—</span>
                      )}
                    </td>
                  );
                })}
              </tr>
            );
          })}
        </tbody>
      </table>
      <p className="border-t border-line px-4 py-3 text-[13px] leading-5 text-ink-3">
        Percentages compare with the same provider’s on-demand price. Committed prices today come from providers that
        publish them (Microsoft Azure); a dash means the provider reports no such tier.
      </p>
    </div>
  );
}

const day = (s: string) => Date.UTC(+s.slice(0, 4), +s.slice(5, 7) - 1, +s.slice(8, 10));
const label = (ms: number) => new Date(ms).toLocaleDateString("en-US", { month: "short", day: "numeric", timeZone: "UTC" });

/**
 * The market's price of one item day by day: cheapest, median and, for GPUs, priciest. Snapshots
 * begin the day the worker first recorded them, so a new item shows today's numbers until a
 * second day arrives.
 */
export function HistoryPanel({ history, unit, hasMax }: { history: SnapshotPoint[]; unit: string; hasMax: boolean }) {
  const fmt = (v: number) => (unit === "gpu" ? formatPrice(v) : formatPrice(v));
  const last = history.at(-1);
  if (!last) return <p className="p-6 text-[14px] text-ink-3">No snapshots recorded yet.</p>;
  const series = [
    { key: "min" as const, name: "Cheapest", color: "var(--lo)" },
    { key: "median" as const, name: "Median", color: "var(--s-7)" },
    ...(hasMax ? [{ key: "max" as const, name: "Priciest", color: "var(--hi)" }] : []),
  ];
  const first = history[0];
  const days = history.length;
  return (
    <div>
      <div className={`grid border-b border-line ${hasMax ? "grid-cols-3" : "grid-cols-2"}`}>
        {series.map((s) => (
          <div key={s.key} className="border-line px-5 py-4 not-first:border-l">
            <span className="flex items-center gap-2 text-[14px] text-ink-2">
              <span className="size-2.5" style={{ background: s.color }} aria-hidden />
              {s.name}
            </span>
            <span className="block font-display text-[26px] leading-9 font-semibold tracking-[-0.03em] text-ink tabular">
              {fmt((last[s.key] ?? 0) as number)}
            </span>
            {days > 1 && first[s.key] != null && (first[s.key] as number) > 0 && (
              <span className={`block font-mono text-[13px] ${(last[s.key] as number) <= (first[s.key] as number) ? "text-lo" : "text-hi"}`}>
                {(((last[s.key] as number) / (first[s.key] as number) - 1) * 100).toFixed(1)}% since {label(day(first.day))}
              </span>
            )}
          </div>
        ))}
      </div>
      {days > 1 ? (
        <HistoryLines history={history} series={series} />
      ) : (
        <p className="px-5 py-4 text-[13px] leading-5 text-ink-3">
          Recording began {label(day(last.day))}. The line appears from the second day, and a projection needs about 30 days
          of history; none is drawn until then.
        </p>
      )}
    </div>
  );
}

function HistoryLines({ history, series }: { history: SnapshotPoint[]; series: { key: "min" | "median" | "max"; name: string; color: string }[] }) {
  const t0 = day(history[0].day);
  const t1 = day(history[history.length - 1].day);
  const values = history.flatMap((p) => series.map((s) => (p[s.key] ?? 0) as number));
  const [lo, hi] = [Math.min(...values) * 0.95, Math.max(...values) * 1.05];
  const sx = (t: number) => r2(pad.left + ((t - t0) / (t1 - t0 || 1)) * (W - pad.left - pad.right));
  const sy = (v: number) => r2(H - pad.bottom - ((v - lo) / (hi - lo || 1)) * (H - pad.top - pad.bottom));
  return (
    <div className="p-4">
      <svg viewBox={`0 0 ${W} ${H}`} className="h-auto w-full" role="img" aria-label="Daily price history">
        {[0, 0.25, 0.5, 0.75, 1].map((f) => {
          const v = lo + (hi - lo) * f;
          return (
            <g key={f}>
              <line x1={pad.left} x2={W - pad.right} y1={sy(v)} y2={sy(v)} stroke="var(--line)" />
              <text x={pad.left - 8} y={sy(v) + 4} textAnchor="end" className="fill-ink-3 font-mono text-[11px]">
                {shortPrice(v)}
              </text>
            </g>
          );
        })}
        {[t0, t1].map((t, i) => (
          <text key={t} x={sx(t)} y={H - 10} textAnchor={i ? "end" : "start"} className="fill-ink-3 font-mono text-[11px]">
            {label(t)}
          </text>
        ))}
        {series.map((s) => (
          <polyline
            key={s.key}
            points={history.map((p) => `${sx(day(p.day))},${sy((p[s.key] ?? 0) as number)}`).join(" ")}
            fill="none"
            stroke={s.color}
            strokeWidth={2}
            strokeLinejoin="round"
          />
        ))}
      </svg>
    </div>
  );
}

/** Each provider's price for a model, cheapest first: the shape of the market for one model. */
export function Ladder({ curve }: { curve: ModelCurve }) {
  const rungs = curve.ladder;
  if (rungs.length === 0) return <p className="p-6 text-[14px] text-ink-3">No provider sells this model at a price.</p>;
  const max = Math.max(...rungs.map((r) => r.blended_per_mtok));
  const cheapest = rungs[0].blended_per_mtok;
  return (
    <ol className="flex flex-col py-1">
      {rungs.map((r, i) => (
        <li key={r.organization.slug} className="grid grid-cols-[minmax(130px,200px)_1fr_auto] items-center gap-3 px-4 py-1.5 text-[14px]">
          <span className="flex min-w-0 items-center gap-2">
            <Logo name={r.organization.name} src={r.organization.logo_url} size="xs" />
            <Link href={`/organizations/${r.organization.slug}`} className="truncate text-ink hover:text-s-7">
              {r.organization.name}
            </Link>
          </span>
          <span className="h-4 bg-field" aria-hidden>
            <span
              className="block h-full"
              style={{ width: `${Math.max(1, Math.round((r.blended_per_mtok / max) * 1000) / 10)}%`, background: i === 0 ? "var(--lo)" : "var(--s-4)" }}
            />
          </span>
          <span className="w-28 text-right font-mono text-[13px] text-ink tabular">
            {formatPrice(r.blended_per_mtok)}
            {i > 0 && cheapest > 0 && <span className="block text-[12px] text-ink-3">{(r.blended_per_mtok / cheapest).toFixed(1)}× cheapest</span>}
          </span>
        </li>
      ))}
    </ol>
  );
}
