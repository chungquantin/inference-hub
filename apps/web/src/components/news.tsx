import type { NewsItem } from "@inference-hub/api-client";
import { HyperLink } from "@/components/hyper-link";
import { Badge } from "@/components/ui/primitives";
import { formatDate } from "@/lib/format";

/** One news item: date in a mono column, headline, summary and the organizations it is about. */
export function NewsCard({ item, children }: { item: NewsItem; children?: React.ReactNode }) {
  return (
    <article className="grid gap-x-6 gap-y-1 px-4 py-4 sm:grid-cols-[88px_minmax(0,1fr)]">
      <div className="text-[13px] leading-6 text-ink-3">
        <time dateTime={item.published_at}>{formatDate(item.published_at)}</time>
      </div>
      <div className="flex min-w-0 flex-col gap-1.5">
        <h3 className="text-[16px] leading-6 font-medium text-ink">
          <a href={item.url} rel="noopener" target="_blank" className="hover:text-s-7 hover:underline">
            {item.title}
          </a>
        </h3>
        {item.summary ? (
          <p className="line-clamp-2 text-[14px] leading-5 text-ink-2">{item.summary}</p>
        ) : (
          item.enrichment?.summary_ai && <AiSummary text={item.enrichment.summary_ai} />
        )}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 pt-0.5">
          {item.publisher && <span className="text-[13px] text-ink-3">{item.publisher}</span>}
          {item.organizations.map((org) => (
            <HyperLink
              key={org.slug}
              type="org"
              slug={org.slug}
              name={org.name}
              logoUrl={org.logo_url}
              size="xs"
              className="inline-flex h-6 items-center gap-1.5 bg-field pr-2 text-[13px] text-ink-2 hover:bg-s-2 hover:text-ink"
            />
          ))}
        </div>
        {children}
      </div>
    </article>
  );
}

/** A machine-written summary, always labelled as one. */
export function AiSummary({ text }: { text: string }) {
  return (
    <p className="line-clamp-2 text-[14px] leading-5 text-ink-2">
      <span className="mr-1.5 text-[12px] font-semibold text-ink-3">AI summary</span>
      {text}
    </p>
  );
}

/** The LLM's triage, for editors: relevance, suggested verdict and confidence. */
export function TriageBadges({ item }: { item: NewsItem }) {
  const e = item.enrichment;
  if (!e) return <span className="font-mono text-[12px] text-ink-3">not triaged</span>;
  const relevance = e.relevance === "core" ? "brand" : e.relevance === "adjacent" ? "neutral" : "down";
  return (
    <div className="flex flex-wrap items-center gap-2" title={`Triaged by ${e.model}`}>
      <Badge tone={relevance}>{e.relevance.replace("_", " ")}</Badge>
      <Badge tone="neutral">{e.topic.replace("_", " ")}</Badge>
      <Badge tone={e.verdict === "approve" ? "up" : "warn"}>
        AI suggests {e.verdict} · {Math.round(e.confidence * 100)}%
      </Badge>
    </div>
  );
}

export function NewsList({ items, empty = "No news yet." }: { items: NewsItem[]; empty?: string }) {
  if (items.length === 0) return <p className="text-[14px] text-ink-3">{empty}</p>;
  return (
    <div className="divide-y divide-line border border-line bg-wash">
      {items.map((item) => (
        <NewsCard key={item.id} item={item} />
      ))}
    </div>
  );
}
