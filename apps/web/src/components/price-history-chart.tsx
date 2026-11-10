import type { PricePoint } from "@inference-hub/api-client";
import { formatDate, formatPrice } from "@/lib/format";

const WIDTH = 640;
const HEIGHT = 220;
const PAD = { top: 12, right: 16, bottom: 28, left: 64 };
const COLORS = ["#2563eb", "#dc2626", "#16a34a", "#d97706", "#7c3aed", "#0891b2", "#db2777", "#4b5563"];

type Series = { label: string; currency: string; points: { t: number; v: number }[] };

/** The price a point is compared on: output tokens for APIs, the hour for instances. */
function priceOf(p: PricePoint): number | null {
  return p.price_per_hour ?? p.price_output_per_mtok ?? null;
}

/** One series per provider and configuration, oldest point first. */
export function toSeries(points: PricePoint[]): Series[] {
  const byKey = new Map<string, Series>();
  for (const p of points) {
    const v = priceOf(p);
    if (v == null) continue;
    const label = p.configuration ? `${p.provider.name} · ${p.configuration}` : p.provider.name;
    const series = byKey.get(label) ?? { label, currency: p.currency, points: [] };
    series.points.push({ t: Date.parse(p.observed_at), v });
    byKey.set(label, series);
  }
  return [...byKey.values()].map((s) => ({ ...s, points: s.points.sort((a, b) => a.t - b.t) }));
}

/**
 * Price over time as step lines: a price holds until the next observation,
 * and the last one holds until today.
 */
export function PriceHistoryChart({
  points,
  unit,
  now,
}: {
  points: PricePoint[];
  unit: string;
  /** Where the last price ends, in epoch milliseconds: the time the page was rendered. */
  now: number;
}) {
  const series = toSeries(points);
  // A chart needs a change to show; one observation per series is already in the table.
  if (!series.some((s) => s.points.length > 1)) return null;

  const times = series.flatMap((s) => s.points.map((p) => p.t));
  const values = series.flatMap((s) => s.points.map((p) => p.v));
  const t0 = Math.min(...times);
  const t1 = Math.max(now, ...times);
  const vMax = Math.max(...values) * 1.1 || 1;
  const x = (t: number) => PAD.left + ((t - t0) / (t1 - t0 || 1)) * (WIDTH - PAD.left - PAD.right);
  const y = (v: number) => HEIGHT - PAD.bottom - (v / vMax) * (HEIGHT - PAD.top - PAD.bottom);
  const currency = series[0].currency;
  const ticks = [0, 0.5, 1].map((f) => vMax * f);

  // Move to the first point, then for each price: a horizontal run until the
  // next observation (or today), and a vertical step to the next price.
  const path = (s: Series) => {
    const parts: string[] = [];
    s.points.forEach((p, i) => {
      const end = s.points[i + 1]?.t ?? t1;
      parts.push(i === 0 ? `M${x(p.t).toFixed(1)},${y(p.v).toFixed(1)}` : `V${y(p.v).toFixed(1)}`);
      parts.push(`H${x(end).toFixed(1)}`);
    });
    return parts.join(" ");
  };

  return (
    <figure className="flex flex-col gap-2">
      <svg
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        className="w-full max-w-2xl"
        role="img"
        aria-label={`Price history, ${unit}`}
      >
        {ticks.map((v) => (
          <g key={v}>
            <line
              x1={PAD.left}
              x2={WIDTH - PAD.right}
              y1={y(v)}
              y2={y(v)}
              className="stroke-zinc-200 dark:stroke-zinc-800"
            />
            <text x={PAD.left - 6} y={y(v) + 4} textAnchor="end" className="fill-zinc-500 text-[11px]">
              {formatPrice(v, currency)}
            </text>
          </g>
        ))}
        <text x={PAD.left} y={HEIGHT - 8} className="fill-zinc-500 text-[11px]">
          {formatDate(new Date(t0).toISOString())}
        </text>
        <text x={WIDTH - PAD.right} y={HEIGHT - 8} textAnchor="end" className="fill-zinc-500 text-[11px]">
          Today
        </text>
        {series.map((s, i) => (
          <path
            key={s.label}
            d={path(s)}
            fill="none"
            stroke={COLORS[i % COLORS.length]}
            strokeWidth={2}
          />
        ))}
      </svg>
      <figcaption className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-zinc-600 dark:text-zinc-400">
        <span className="text-zinc-500">{unit}:</span>
        {series.map((s, i) => (
          <span key={s.label} className="flex items-center gap-1.5">
            <span className="inline-block size-2.5 rounded-sm" style={{ background: COLORS[i % COLORS.length] }} />
            {s.label}
          </span>
        ))}
      </figcaption>
    </figure>
  );
}
