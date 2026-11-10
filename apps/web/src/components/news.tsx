import Link from "next/link";
import type { NewsItem } from "@inference-hub/api-client";
import { Logo } from "@/components/ui/logo";
import { formatDate } from "@/lib/format";

/** One news item: who it is about, the headline linking out, publisher and date. */
export function NewsCard({ item, children }: { item: NewsItem; children?: React.ReactNode }) {
  const lead = item.organizations[0];
  return (
    <article className="flex gap-3.5 px-4 py-4">
      <div className="pt-0.5">
        {lead ? (
          <Logo name={lead.name} src={lead.logo_url} size="md" />
        ) : (
          <Logo name={item.publisher ?? item.title} size="md" />
        )}
      </div>
      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <h3 className="font-medium leading-snug text-ink">
          <a href={item.url} rel="noopener" target="_blank" className="hover:text-accent">
            {item.title}
          </a>
        </h3>
        {item.summary && <p className="line-clamp-2 text-sm text-ink-2">{item.summary}</p>}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-ink-3">
          {item.publisher && <span>{item.publisher}</span>}
          <time dateTime={item.published_at}>{formatDate(item.published_at)}</time>
          {item.organizations.map((org) => (
            <Link
              key={org.slug}
              href={`/organizations/${org.slug}`}
              className="inline-flex items-center gap-1.5 rounded-full bg-inset py-0.5 pr-2 pl-0.5 text-ink-2 hover:text-ink"
            >
              <Logo name={org.name} src={org.logo_url} size="sm" />
              {org.name}
            </Link>
          ))}
        </div>
        {children}
      </div>
    </article>
  );
}

export function NewsList({ items, empty = "No news yet." }: { items: NewsItem[]; empty?: string }) {
  if (items.length === 0) return <p className="text-sm text-ink-3">{empty}</p>;
  return (
    <div className="divide-y divide-line overflow-hidden rounded-[10px] bg-surface shadow-elev-1">
      {items.map((item) => (
        <NewsCard key={item.id} item={item} />
      ))}
    </div>
  );
}
