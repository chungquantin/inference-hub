import Link from "next/link";
import type { NewsItem } from "@inference-hub/api-client";
import { Logo } from "@/components/ui/logo";
import { formatDate } from "@/lib/format";

/** One news item: date in a mono column, headline, summary and the organizations it is about. */
export function NewsCard({ item, children }: { item: NewsItem; children?: React.ReactNode }) {
  return (
    <article className="grid gap-x-6 gap-y-1 px-4 py-4 sm:grid-cols-[96px_minmax(0,1fr)]">
      <div className="font-mono text-[12px] leading-5 text-ink-3">
        <time dateTime={item.published_at}>{formatDate(item.published_at)}</time>
      </div>
      <div className="flex min-w-0 flex-col gap-1.5">
        <h3 className="text-[16px] leading-6 text-ink">
          <a href={item.url} rel="noopener" target="_blank" className="hover:text-s-7 hover:underline">
            {item.title}
          </a>
        </h3>
        {item.summary && <p className="line-clamp-2 text-[14px] leading-5 text-ink-2">{item.summary}</p>}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 pt-0.5">
          {item.publisher && <span className="font-mono text-[12px] text-ink-3">{item.publisher}</span>}
          {item.organizations.map((org) => (
            <Link
              key={org.slug}
              href={`/organizations/${org.slug}`}
              className="inline-flex h-6 items-center gap-1.5 bg-field pr-2 font-mono text-[12px] text-ink-2 hover:bg-s-2 hover:text-ink"
            >
              <Logo name={org.name} src={org.logo_url} size="xs" />
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
  if (items.length === 0) return <p className="font-mono text-[13px] text-ink-3">{empty}</p>;
  return (
    <div className="divide-y divide-line border border-line bg-wash">
      {items.map((item) => (
        <NewsCard key={item.id} item={item} />
      ))}
    </div>
  );
}
