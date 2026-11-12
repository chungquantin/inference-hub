"use client";

import { useState } from "react";

/** Compact price label: $0.025, $1.20, $12.0. */
export function shortPrice(v: number): string {
  if (v >= 100) return `$${Math.round(v)}`;
  if (v >= 10) return `$${v.toFixed(1)}`;
  if (v >= 1) return `$${v.toFixed(2)}`;
  if (v >= 0.1) return `$${v.toFixed(3)}`;
  return `$${v.toPrecision(2)}`;
}

/** Brand ramp first, then distinct hues, for series such as makers. */
export const SERIES = ["#3a5bd9", "#0f7a48", "#cc3a21", "#c98a00", "#7a3fb0", "#0b8bb5", "#b5439b", "#55534b"];

export type ScatterPoint = {
  id: string;
  x: number;
  y: number;
  r: number;
  color: string;
  label: string;
  detail: string;
};

/**
 * A scatter on log axes. Hovering or focusing a dot shows its label; clicking selects it.
 * Axes are log10 because prices span orders of magnitude.
 */
export function LogScatter({
  points,
  xLabel,
  yLabel,
  xFormat = shortPrice,
  yFormat = (v: number) => `${v < 10 ? v.toFixed(1) : Math.round(v)}×`,
  selected,
  onSelect,
  height = 360,
  labelFrom,
}: {
  points: ScatterPoint[];
  xLabel: string;
  yLabel: string;
  xFormat?: (v: number) => string;
  yFormat?: (v: number) => string;
  selected?: string | null;
  onSelect?: (id: string) => void;
  height?: number;
  /** Draw each dot's name beside it when its radius is at least this. */
  labelFrom?: number;
}) {
  const [hover, setHover] = useState<string | null>(null);
  const W = 720;
  const pad = { top: 16, right: 20, bottom: 44, left: 52 };
  if (points.length === 0) return <p className="p-6 text-[14px] text-ink-3">Nothing matches these filters.</p>;
  const xs = points.map((p) => Math.log10(p.x));
  const ys = points.map((p) => Math.log10(p.y));
  const [x0, x1] = [Math.floor(Math.min(...xs)), Math.max(Math.ceil(Math.max(...xs)), Math.floor(Math.min(...xs)) + 1)];
  const [y0, y1] = [Math.floor(Math.min(...ys)), Math.max(Math.ceil(Math.max(...ys)), Math.floor(Math.min(...ys)) + 1)];
  // Rounded: server and browser Math.log10 can differ in the last digit, which breaks hydration.
  const round = (n: number) => Math.round(n * 100) / 100;
  const sx = (v: number) => round(pad.left + ((Math.log10(v) - x0) / (x1 - x0)) * (W - pad.left - pad.right));
  const sy = (v: number) => round(height - pad.bottom - ((Math.log10(v) - y0) / (y1 - y0)) * (height - pad.top - pad.bottom));
  const decades = (a: number, b: number) => Array.from({ length: b - a + 1 }, (_, i) => a + i);
  const active = points.find((p) => p.id === (hover ?? selected));
  return (
    <div className="relative">
      <svg viewBox={`0 0 ${W} ${height}`} className="h-auto w-full" role="group" aria-label={`${yLabel} against ${xLabel}`}>
        {decades(x0, x1).map((d) => (
          <g key={`x${d}`}>
            <line x1={sx(10 ** d)} x2={sx(10 ** d)} y1={pad.top} y2={height - pad.bottom} stroke="var(--line)" />
            <text x={sx(10 ** d)} y={height - pad.bottom + 16} textAnchor="middle" className="fill-ink-3 font-mono text-[11px]">
              {xFormat(10 ** d)}
            </text>
          </g>
        ))}
        {decades(y0, y1).map((d) => (
          <g key={`y${d}`}>
            <line x1={pad.left} x2={W - pad.right} y1={sy(10 ** d)} y2={sy(10 ** d)} stroke="var(--line)" />
            <text x={pad.left - 8} y={sy(10 ** d) + 4} textAnchor="end" className="fill-ink-3 font-mono text-[11px]">
              {yFormat(10 ** d)}
            </text>
          </g>
        ))}
        <text x={(pad.left + W - pad.right) / 2} y={height - 6} textAnchor="middle" className="fill-ink-2 text-[12px]">
          {xLabel}
        </text>
        <text
          transform={`translate(12 ${(pad.top + height - pad.bottom) / 2}) rotate(-90)`}
          textAnchor="middle"
          className="fill-ink-2 text-[12px]"
        >
          {yLabel}
        </text>
        {points.map((p) => (
          <circle
            key={p.id}
            cx={sx(p.x)}
            cy={sy(p.y)}
            r={round(p.r)}
            fill={p.color}
            fillOpacity={active ? (active.id === p.id ? 0.95 : 0.25) : 0.6}
            stroke={selected === p.id ? "var(--ink)" : "var(--wash)"}
            strokeWidth={selected === p.id ? 2 : 1}
            tabIndex={0}
            role="button"
            aria-label={`${p.label}: ${p.detail}`}
            className="cursor-pointer outline-none focus-visible:stroke-ink"
            onMouseEnter={() => setHover(p.id)}
            onMouseLeave={() => setHover(null)}
            onFocus={() => setHover(p.id)}
            onBlur={() => setHover(null)}
            onClick={() => onSelect?.(p.id)}
            onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onSelect?.(p.id)}
          />
        ))}
        {labelFrom != null &&
          points
            .filter((p) => p.r >= labelFrom)
            .map((p) => (
              <text
                key={`l${p.id}`}
                x={sx(p.x) + round(p.r) + 4}
                y={sy(p.y) + 4}
                className="pointer-events-none fill-ink text-[11px]"
                paintOrder="stroke"
                stroke="var(--wash)"
                strokeWidth={3}
              >
                {p.label}
              </text>
            ))}
      </svg>
      {active && (
        <div
          className="pointer-events-none absolute top-2 left-14 max-w-[70%] border border-line-strong bg-wash px-3 py-2 text-[13px]"
          role="status"
        >
          <span className="font-medium text-ink">{active.label}</span>
          <span className="block font-mono text-[12px] text-ink-2">{active.detail}</span>
        </div>
      )}
    </div>
  );
}

/**
 * Min · median · max on one log track: the green tick is the cheapest, the
 * blue tick the median, the band the range. Shows how far the market stretches.
 */
export function RangeBar({
  min,
  median,
  max,
  domain,
  label,
}: {
  min: number;
  median: number;
  max: number;
  domain: [number, number];
  label: string;
}) {
  const [lo, hi] = [Math.log10(domain[0]), Math.log10(domain[1])];
  const pos = (v: number) => `${Math.min(100, Math.max(0, ((Math.log10(v) - lo) / (hi - lo || 1)) * 100)).toFixed(2)}%`;
  return (
    <span role="img" aria-label={label} className="relative block h-3 w-full min-w-[120px] bg-field">
      <span
        className="absolute top-1/2 h-1.5 -translate-y-1/2 bg-s-3"
        style={{ left: pos(min), width: `calc(${pos(max)} - ${pos(min)})` }}
      />
      <span className="absolute top-0 h-3 w-[3px] bg-lo" style={{ left: pos(min) }} />
      <span className="absolute top-0 h-3 w-[2px] bg-s-8" style={{ left: pos(median) }} />
    </span>
  );
}

/** A centered bar around 1.0 on a log scale: left is cheaper than the market, right pricier. */
export function IndexBar({ value, label }: { value: number | null | undefined; label: string }) {
  if (value == null) return <span className="text-ink-3">—</span>;
  const f = Math.max(-1, Math.min(1, Math.log2(value))); // the track covers 0.5× to 2×
  const cheap = f < 0;
  return (
    <span role="img" aria-label={label} className="relative block h-3 w-full min-w-[110px] bg-field">
      <span className="absolute top-0 left-1/2 h-3 w-px bg-ink-3" />
      <span
        className={`absolute top-0.5 h-2 ${cheap ? "bg-lo" : "bg-hi"}`}
        style={cheap ? { right: "50%", width: `${(-f * 50).toFixed(2)}%` } : { left: "50%", width: `${(f * 50).toFixed(2)}%` }}
      />
    </span>
  );
}

/** Vertical histogram with two series: all models, and those sold by 2+ providers. */
export function Histogram({
  buckets,
}: {
  buckets: { from: number; to: number; models: number; contested: number }[];
}) {
  const used = buckets.filter((b) => b.models > 0);
  if (used.length === 0) return null;
  const shown = buckets.slice(buckets.indexOf(used[0]), buckets.indexOf(used[used.length - 1]) + 1);
  const max = Math.max(...shown.map((b) => b.models));
  const W = 720;
  const H = 220;
  const pad = { top: 18, right: 8, bottom: 30, left: 8 };
  const bw = (W - pad.left - pad.right) / shown.length;
  const h = (v: number) => (v / max) * (H - pad.top - pad.bottom);
  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="h-auto w-full" role="group" aria-label="Chat models by cheapest price per million tokens">
      {shown.map((b, i) => {
        const x = pad.left + i * bw;
        return (
          <g key={b.from}>
            <title>{`${shortPrice(b.from)} to ${shortPrice(b.to)}: ${b.models.toLocaleString()} models, ${b.contested} on 2+ providers`}</title>
            <rect x={x + 3} y={H - pad.bottom - h(b.models)} width={bw - 6} height={h(b.models)} fill="var(--s-3)" />
            <rect x={x + 3} y={H - pad.bottom - h(b.contested)} width={bw - 6} height={h(b.contested)} fill="var(--s-8)" />
            <text x={x + bw / 2} y={H - pad.bottom - h(b.models) - 4} textAnchor="middle" className="fill-ink-2 font-mono text-[11px]">
              {b.models.toLocaleString()}
            </text>
            <text x={x + bw / 2} y={H - 10} textAnchor="middle" className="fill-ink-3 font-mono text-[10px]">
              {shortPrice(b.from)}
            </text>
          </g>
        );
      })}
    </svg>
  );
}

/** Heat color for a price relative to the market median: green below 1, red above. */
export function heat(vsMedian: number): { bg: string; fg: string } {
  const f = Math.max(-1, Math.min(1, Math.log2(vsMedian) / 1.5));
  const mix = Math.abs(f);
  const base = f < 0 ? "15,122,72" : "204,58,33";
  return { bg: `rgba(${base},${(0.1 + mix * 0.62).toFixed(2)})`, fg: mix > 0.55 ? "#fff" : "var(--ink)" };
}
