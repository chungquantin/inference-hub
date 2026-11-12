"use client";

import { useState } from "react";
import type { MetricBoard } from "@inference-hub/api-client";
import { Logo } from "@/components/ui/logo";
import { SERIES } from "@/components/rankings/charts";
import { formatMetric } from "@/lib/metrics";

const W = 760;
const H = 340;
const pad = { top: 14, right: 16, bottom: 30, left: 62 };

/** A round axis step that gives 4–6 ticks over [0, max]. */
function niceTicks(max: number): number[] {
  if (max <= 0) return [0, 1];
  const raw = max / 4;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = ([1, 2, 2.5, 5, 10].find((m) => m * mag >= raw) ?? 10) * mag;
  const out: number[] = [];
  for (let v = 0; v <= max + step * 0.5; v += step) out.push(v);
  return out;
}

const r2 = (n: number) => Math.round(n * 100) / 100;
const day = (s: string) => Date.UTC(+s.slice(0, 4), +s.slice(5, 7) - 1, +s.slice(8, 10));
const label = (ms: number) => new Date(ms).toLocaleDateString("en-US", { month: "short", day: "numeric", timeZone: "UTC" });

/**
 * The featured chart of a metric: the leaders' history as lines with a legend that
 * doubles as a ranking, as on Token Terminal. A metric with no history yet shows
 * bars of its leaders instead; lines appear as days of readings accumulate.
 */
export function HistoryChart({ board }: { board: MetricBoard }) {
  const [hover, setHover] = useState<string | null>(null);
  const unit = board.unit;
  const lines = board.lines;
  const hasHistory = lines.some((l) => l.points.length >= 2);
  const color = (i: number) => SERIES[i % SERIES.length];

  return (
    <div className="grid gap-0 lg:grid-cols-[1fr_300px]">
      <div className="min-w-0 border-line p-4 lg:border-r">
        {hasHistory ? (
          <Lines board={board} hover={hover} color={color} />
        ) : (
          <Bars board={board} hover={hover} color={color} />
        )}
        {!hasHistory && (
          <p className="mt-2 max-w-[62ch] text-[13px] leading-5 text-ink-3">
            This source reports today’s value only. Lines appear as the worker records one reading a day
            {board.as_of ? `, from ${label(day(board.as_of))}` : ""}.
          </p>
        )}
      </div>
      <div className="flex flex-col">
        <div className="flex items-center justify-between border-t border-b border-line px-4 py-2 text-[13px] text-ink-3 lg:border-t-0">
          <span>{board.subject_type === "model" ? "Model" : board.subject_type === "project" ? "Project" : "Organization"}</span>
          <span>Latest</span>
        </div>
        <ul className="flex flex-col py-1">
          {lines.map((l, i) => (
            <li
              key={l.subject_key}
              onMouseEnter={() => setHover(l.subject_key)}
              onMouseLeave={() => setHover(null)}
              className={`flex items-center gap-2.5 px-4 py-1.5 text-[14px] leading-5 ${
                hover && hover !== l.subject_key ? "opacity-40" : ""
              }`}
            >
              <span className="size-2.5 shrink-0" style={{ background: color(i) }} aria-hidden />
              {l.organization && <Logo name={l.organization.name} src={l.organization.logo_url} size="xs" />}
              <span className="min-w-0 flex-1 truncate text-ink" title={l.name}>
                {l.name}
              </span>
              <span className="font-mono text-[13px] text-ink-2 tabular">
                {formatMetric(unit, l.points.at(-1)?.value)}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

function Lines({
  board,
  hover,
  color,
}: {
  board: MetricBoard;
  hover: string | null;
  color: (i: number) => string;
}) {
  const all = board.lines.flatMap((l) => l.points.map((p) => [day(p.day), p.value] as const));
  const [t0, t1] = [Math.min(...all.map((p) => p[0])), Math.max(...all.map((p) => p[0]))];
  const ticks = niceTicks(Math.max(...all.map((p) => p[1])));
  const top = ticks[ticks.length - 1];
  const sx = (t: number) => r2(pad.left + ((t - t0) / (t1 - t0 || 1)) * (W - pad.left - pad.right));
  const sy = (v: number) => r2(H - pad.bottom - (v / top) * (H - pad.top - pad.bottom));
  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="h-auto w-full" role="img" aria-label={`${board.label}, history of the leaders`}>
      {ticks.map((v) => (
        <g key={v}>
          <line x1={pad.left} x2={W - pad.right} y1={sy(v)} y2={sy(v)} stroke="var(--line)" />
          <text x={pad.left - 8} y={sy(v) + 4} textAnchor="end" className="fill-ink-3 font-mono text-[11px]">
            {formatMetric(board.unit, v).replace(" tok/s", "")}
          </text>
        </g>
      ))}
      {[t0, (t0 + t1) / 2, t1].map((t, i) => (
        <text
          key={i}
          x={sx(t)}
          y={H - 8}
          textAnchor={i === 0 ? "start" : i === 2 ? "end" : "middle"}
          className="fill-ink-3 font-mono text-[11px]"
        >
          {label(t)}
        </text>
      ))}
      {board.lines.map((l, i) => (
        <polyline
          key={l.subject_key}
          points={l.points.map((p) => `${sx(day(p.day))},${sy(p.value)}`).join(" ")}
          fill="none"
          stroke={color(i)}
          strokeWidth={hover === l.subject_key ? 2.5 : 1.75}
          strokeLinejoin="round"
          opacity={hover && hover !== l.subject_key ? 0.2 : 1}
        />
      ))}
    </svg>
  );
}

function Bars({
  board,
  hover,
  color,
}: {
  board: MetricBoard;
  hover: string | null;
  color: (i: number) => string;
}) {
  const rows = board.lines.map((l, i) => ({ l, i, v: l.points.at(-1)?.value ?? 0 }));
  const max = Math.max(...rows.map((r) => r.v), 1);
  return (
    <ol className="flex flex-col gap-1.5" aria-label={`${board.label}, the leaders today`}>
      {rows.map(({ l, i, v }) => (
        <li
          key={l.subject_key}
          className={`grid grid-cols-[minmax(90px,150px)_1fr_auto] items-center gap-3 text-[13px] ${
            hover && hover !== l.subject_key ? "opacity-30" : ""
          }`}
        >
          <span className="truncate text-right text-ink" title={l.name}>
            {l.name}
          </span>
          <span className="h-5 bg-field">
            <span className="block h-full" style={{ width: `${Math.max(1, Math.round((v / max) * 1000) / 10)}%`, background: color(i) }} />
          </span>
          <span className="w-24 font-mono text-[12px] text-ink-2 tabular">{formatMetric(board.unit, v)}</span>
        </li>
      ))}
    </ol>
  );
}
