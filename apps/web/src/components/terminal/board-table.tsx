"use client";

import Link from "next/link";
import { useMemo, useState } from "react";
import type { MetricBoard, MetricRow } from "@inference-hub/api-client";
import { Logo } from "@/components/ui/logo";
import { changeTone, formatChange, formatMetric, subjectHref } from "@/lib/metrics";
import { Spark } from "./spark";

type Key = "rank" | "name" | "value" | "change_7d" | "change_30d" | "change_90d" | "share";

const get = (r: MetricRow, k: Key): number | string | null | undefined => (k === "name" ? r.name.toLowerCase() : r[k]);

/** Every subject of a metric: sortable, filterable, with change over 7, 30 and 90 days. */
export function BoardTable({ board }: { board: MetricBoard }) {
  const [q, setQ] = useState("");
  const [sort, setSort] = useState<{ key: Key; dir: 1 | -1 }>({ key: "rank", dir: 1 });
  const hasShare = board.rows.some((r) => r.share != null);
  const hasChange = board.rows.some((r) => r.change_7d != null || r.change_30d != null || r.change_90d != null);

  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    const filtered = needle
      ? board.rows.filter((r) => `${r.name} ${r.organization?.name ?? ""}`.toLowerCase().includes(needle))
      : board.rows;
    return [...filtered].sort((a, b) => {
      const [x, y] = [get(a, sort.key), get(b, sort.key)];
      // Rows without a value go last, whichever way the column sorts.
      if (x == null || y == null) return x == null ? (y == null ? 0 : 1) : -1;
      return (x < y ? -1 : x > y ? 1 : 0) * sort.dir;
    });
  }, [board.rows, q, sort]);

  const head = (key: Key, text: string, numeric = true) => (
    <th
      scope="col"
      aria-sort={sort.key === key ? (sort.dir === 1 ? "ascending" : "descending") : "none"}
      className={`h-10 px-3 text-[13px] font-medium whitespace-nowrap ${numeric ? "text-right" : "text-left"}`}
    >
      <button
        type="button"
        onClick={() => setSort((s) => ({ key, dir: s.key === key ? (-s.dir as 1 | -1) : key === "name" ? 1 : key === "rank" ? 1 : -1 }))}
        className={`inline-flex items-center gap-1 hover:text-ink ${sort.key === key ? "text-ink" : "text-ink-3"}`}
      >
        {text}
        <span aria-hidden className="w-2 text-[10px]">
          {sort.key === key ? (sort.dir === 1 ? "▲" : "▼") : ""}
        </span>
      </button>
    </th>
  );

  return (
    <div>
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-4 py-3">
        <label className="flex items-center gap-2 text-[13px] text-ink-3">
          <span className="sr-only">Filter {board.label}</span>
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Filter by name"
            className="h-9 w-56 border border-line-strong bg-wash px-3 text-[14px] text-ink outline-none focus-visible:border-s-7"
          />
        </label>
        <span className="font-mono text-[13px] text-ink-3 tabular">
          {rows.length.toLocaleString("en-US")} of {board.rows.length.toLocaleString("en-US")}
        </span>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full border-collapse text-[14px] leading-5">
          <thead>
            <tr className="border-b border-line bg-paper">
              {head("rank", "#")}
              {head("name", board.subject_type === "model" ? "Model" : board.subject_type === "project" ? "Project" : "Organization", false)}
              {head("value", board.label)}
              {hasChange && head("change_7d", "7d")}
              {hasChange && head("change_30d", "30d")}
              {hasChange && head("change_90d", "90d")}
              {hasShare && head("share", "Share")}
              <th scope="col" className="h-10 px-3 text-left text-[13px] font-medium text-ink-3">
                Last 60 days
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const href = subjectHref(r, board.subject_type);
              return (
                <tr key={r.subject_key} className="border-b border-line last:border-0 hover:bg-paper">
                  <td className="w-12 px-3 py-2 text-right font-mono text-[13px] text-ink-3 tabular">{r.rank}</td>
                  <td className="px-3 py-2">
                    <span className="flex min-w-0 items-center gap-2.5">
                      {r.organization && <Logo name={r.organization.name} src={r.organization.logo_url} size="sm" />}
                      {href ? (
                        <Link href={href} className="min-w-0 truncate font-medium text-ink hover:text-s-7">
                          {r.organization?.name ?? r.name}
                        </Link>
                      ) : (
                        <span className="min-w-0 truncate font-medium text-ink">{r.name}</span>
                      )}
                      {r.organization && r.organization.name !== r.name && (
                        <span className="hidden truncate text-[13px] text-ink-3 sm:inline">{r.name}</span>
                      )}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-right font-mono text-ink tabular">{formatMetric(board.unit, r.value)}</td>
                  {hasChange &&
                    ([r.change_7d, r.change_30d, r.change_90d] as const).map((c, i) => (
                      <td key={i} className={`px-3 py-2 text-right font-mono text-[13px] tabular ${changeTone(c, board.higher_is_better)}`}>
                        {formatChange(c)}
                      </td>
                    ))}
                  {hasShare && (
                    <td className="px-3 py-2 text-right font-mono text-[13px] text-ink-2 tabular">
                      {r.share != null ? `${(r.share * 100).toFixed(r.share < 0.1 ? 1 : 0)}%` : "—"}
                    </td>
                  )}
                  <td className="px-3 py-2">
                    <Spark values={r.spark} />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {rows.length === 0 && <p className="p-6 text-[14px] text-ink-3">Nothing matches “{q}”.</p>}
      </div>
    </div>
  );
}
