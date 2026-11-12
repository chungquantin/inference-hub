import type { Metadata } from "next";
import Link from "next/link";
import type { MetricSummary } from "@inference-hub/api-client";
import { ago } from "@/components/market";
import { Spark } from "@/components/terminal/spark";
import { Logo } from "@/components/ui/logo";
import { LiveDot } from "@/components/ui/pixel";
import { Frame, PageHeader } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { GROUP_ORDER, changeTone, formatChange, formatMetric, metricHref, subjectHref } from "@/lib/metrics";

export const revalidate = 300;

export const metadata: Metadata = {
  title: "Inference terminal",
  description:
    "Every number that describes the inference market beyond price: model downloads, serving speed, open-source momentum. Ranked, with history.",
  alternates: { canonical: "/analytics/metrics" },
};

export default async function TerminalPage() {
  const { data, error } = await api.GET("/v1/market/metrics");
  if (error || !data) throw new Error("Could not load the metrics");
  const newest = data.sources.map((s) => s.last_success_at).filter(Boolean).sort().at(-1);
  const groups = GROUP_ORDER.map((g) => ({ g, items: data.metrics.filter((m) => m.group === g) })).filter((x) => x.items.length);
  return (
    <div className="flex flex-col gap-10">
      <PageHeader
        crumbs={[{ href: "/analytics", label: "Analytics" }, { label: "Terminal" }]}
        title="Terminal"
        lead="Who is used, who is fast and who is gaining ground in inference. Prices are on the market pages; these are the numbers around them, collected daily from public sources."
      >
        {newest && (
          <span className="flex items-center gap-2 font-mono text-[13px] text-ink-3">
            <LiveDot /> updated {ago(newest, new Date().toISOString())}
          </span>
        )}
      </PageHeader>

      {groups.map(({ g, items }) => (
        <section key={g} aria-labelledby={`g-${g}`} className="flex flex-col gap-4">
          <h2 id={`g-${g}`} className="font-display text-[24px] leading-8 font-medium tracking-[-0.035em] text-ink">
            {g}
          </h2>
          <div className="grid gap-4 md:grid-cols-2">
            {items.map((m) => (
              <Card key={m.key} m={m} />
            ))}
          </div>
        </section>
      ))}

      <section aria-labelledby="sources" className="flex flex-col gap-3">
        <h2 id="sources" className="font-display text-[19px] leading-6 font-medium tracking-[-0.03em] text-ink">
          Sources
        </h2>
        <p className="max-w-[62ch] text-[14px] leading-6 text-ink-2">
          Each source is read on a schedule and one value per day is kept, so history grows from the day a metric first
          appears. Hugging Face download counts cover the 30 days before each reading.
        </p>
        <ul className="grid gap-px border border-line bg-line sm:grid-cols-3">
          {data.sources.map((s) => (
            <li key={s.source} className="bg-wash px-4 py-3">
              <span className="block text-[14px] font-medium text-ink">{s.source}</span>
              <span className={`block font-mono text-[13px] ${s.consecutive_failures ? "text-hi" : "text-ink-3"}`}>
                {s.consecutive_failures
                  ? `failing: ${s.last_error ?? "unknown error"}`
                  : `${s.series.toLocaleString("en-US")} series, ${ago(s.last_success_at, new Date().toISOString())}`}
              </span>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

/** A leaderboard: the metric's total when it adds up, then its top five with a bar scaled to the leader. */
function Card({ m }: { m: MetricSummary }) {
  const lead = m.higher_is_better ? m.leaders[0]?.value : Math.max(...m.leaders.map((l) => l.value));
  return (
    <Frame as="article" className="flex flex-col">
      <header className="flex items-start justify-between gap-4 px-4 pt-4 pb-3">
        <div className="min-w-0">
          <h3 className="font-display text-[19px] leading-6 font-medium tracking-[-0.03em] text-ink">
            <Link href={metricHref(m.key)} className="hover:text-s-7">
              {m.label}
            </Link>
          </h3>
          <p className="mt-1 text-[13px] leading-5 text-ink-3">{m.subjects.toLocaleString("en-US")} tracked</p>
        </div>
        {m.total != null && (
          <div className="text-right">
            <span className="block font-display text-[24px] leading-7 font-semibold tracking-[-0.03em] text-ink tabular">
              {formatMetric(m.unit, m.total)}
            </span>
            <span className={`block font-mono text-[13px] tabular ${changeTone(m.total_change_30d, true)}`}>
              {m.total_change_30d != null ? `${formatChange(m.total_change_30d)} 30d` : "total"}
            </span>
          </div>
        )}
      </header>
      <ol className="flex flex-col border-t border-line">
        {m.leaders.map((r) => {
          const href = subjectHref(r, m.subject_type);
          const width = lead ? Math.max(4, Math.round((r.value / lead) * 100)) : 0;
          const name = r.organization?.name ?? r.name;
          return (
            <li key={r.subject_key} className="relative flex items-center gap-3 border-b border-line px-4 py-2 last:border-0">
              <span
                aria-hidden
                className="absolute inset-y-0 left-0 bg-s-1"
                style={{ width: `${m.higher_is_better ? width : 100 - width + 4}%` }}
              />
              <span className="relative w-4 font-mono text-[12px] text-ink-3 tabular">{r.rank}</span>
              <span className="relative flex min-w-0 flex-1 items-center gap-2">
                {r.organization && <Logo name={r.organization.name} src={r.organization.logo_url} size="xs" />}
                {href ? (
                  <Link href={href} className="truncate text-[14px] text-ink hover:text-s-7">
                    {name}
                  </Link>
                ) : (
                  <span className="truncate text-[14px] text-ink" title={r.name}>
                    {name}
                  </span>
                )}
              </span>
              {r.spark.length > 1 && (
                <span className="relative hidden sm:block">
                  <Spark values={r.spark} width={56} height={18} />
                </span>
              )}
              <span className="relative font-mono text-[13px] text-ink tabular">{formatMetric(m.unit, r.value)}</span>
            </li>
          );
        })}
      </ol>
      <footer className="mt-auto flex items-center justify-between border-t border-line px-4 py-2.5 text-[13px]">
        <span className="text-ink-3">{m.source}</span>
        <Link href={metricHref(m.key)} className="font-medium text-s-7 hover:underline">
          Full ranking
        </Link>
      </footer>
    </Frame>
  );
}
