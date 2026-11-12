import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { BoardTable } from "@/components/terminal/board-table";
import { HistoryChart } from "@/components/terminal/history-chart";
import { LiveDot } from "@/components/ui/pixel";
import { Frame, PageHeader } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { changeTone, formatChange, formatMetric } from "@/lib/metrics";

export const revalidate = 300;

type Params = { metric: string };

async function load(metric: string) {
  const { data, response } = await api.GET("/v1/market/metrics/{metric}", {
    params: { path: { metric }, query: { limit: 500 } },
  });
  if (response.status === 404) notFound();
  if (!data) throw new Error("Could not load the metric");
  return data;
}

export async function generateMetadata({ params }: { params: Promise<Params> }): Promise<Metadata> {
  const { metric } = await params;
  const board = await load(metric);
  return {
    title: `${board.label} ranking`,
    description: board.description,
    alternates: { canonical: `/analytics/metrics/${metric}` },
  };
}

export default async function MetricPage({ params }: { params: Promise<Params> }) {
  const { metric } = await params;
  const board = await load(metric);
  const leader = board.rows[0];
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        crumbs={[{ href: "/analytics", label: "Analytics" }, { href: "/analytics/metrics", label: "Terminal" }, { label: board.label }]}
        title={board.label}
        lead={board.description}
      >
        {board.as_of && (
          <span className="flex items-center gap-2 font-mono text-[13px] text-ink-3">
            <LiveDot /> as of {board.as_of}
          </span>
        )}
      </PageHeader>

      <Frame className="grid grid-cols-2 sm:grid-cols-4">
        {[
          board.total != null
            ? { v: formatMetric(board.unit, board.total), l: "Total", s: board.total_change_30d != null ? `${formatChange(board.total_change_30d)} over 30 days` : undefined, tone: changeTone(board.total_change_30d, true) }
            : leader
              ? { v: formatMetric(board.unit, leader.value), l: board.higher_is_better ? "Best" : "Fastest", s: leader.organization?.name ?? leader.name }
              : { v: "—", l: "Best" },
          { v: board.rows.length.toLocaleString("en-US"), l: "Tracked" },
          { v: board.history_days > 0 ? `${board.history_days} days` : "Today", l: "History" },
          { v: board.source, l: "Source" },
        ].map((k) => (
          <div key={k.l} className="min-w-0 border-line px-5 py-4 not-first:border-l">
            <span className={`block font-display font-semibold tracking-[-0.03em] text-ink tabular ${k.l === "Source" ? "text-[16px] leading-8" : "truncate text-[24px] leading-8"}`}>{k.v}</span>
            <span className="block text-[14px] leading-5 text-ink-2">{k.l}</span>
            {k.s && <span className={`block truncate text-[13px] leading-5 ${k.tone ?? "text-ink-3"}`}>{k.s}</span>}
          </div>
        ))}
      </Frame>

      <Frame as="section" className="flex flex-col">
        <header className="flex flex-wrap items-baseline justify-between gap-2 border-b border-line px-4 py-3">
          <h2 className="font-display text-[19px] leading-6 font-medium tracking-[-0.03em] text-ink">
            {board.higher_is_better ? "Top 10" : "Fastest 10"}
          </h2>
          <span className="text-[13px] text-ink-3">{board.history_days > 0 ? "Daily values, last 12 months" : "Latest reading"}</span>
        </header>
        <HistoryChart board={board} />
      </Frame>

      <Frame as="section">
        <BoardTable board={board} />
      </Frame>
    </div>
  );
}
