import Link from "next/link";
import type { NewsItem } from "@inference-hub/api-client";
import { formatDate } from "@/lib/format";

/** One news item: title linking out, publisher, date, summary and organizations. */
export function NewsCard({ item, children }: { item: NewsItem; children?: React.ReactNode }) {
  return (
    <article className="flex flex-col gap-1.5 py-4">
      <div className="flex flex-wrap items-baseline gap-x-2 text-xs text-zinc-500">
        {item.publisher && <span className="font-medium">{item.publisher}</span>}
        <time dateTime={item.published_at}>{formatDate(item.published_at)}</time>
      </div>
      <h3 className="font-medium leading-snug">
        <a href={item.url} rel="noopener" target="_blank" className="hover:underline">
          {item.title}
        </a>
      </h3>
      {item.summary && (
        <p className="line-clamp-3 text-sm text-zinc-600 dark:text-zinc-400">{item.summary}</p>
      )}
      {item.organizations.length > 0 && (
        <ul className="flex flex-wrap gap-1.5 pt-0.5">
          {item.organizations.map((org) => (
            <li key={org.slug}>
              <Link
                href={`/organizations/${org.slug}`}
                className="rounded bg-zinc-100 px-1.5 py-0.5 text-xs text-zinc-700 hover:bg-zinc-200 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700"
              >
                {org.name}
              </Link>
            </li>
          ))}
        </ul>
      )}
      {children}
    </article>
  );
}

export function NewsList({ items, empty = "No news yet." }: { items: NewsItem[]; empty?: string }) {
  if (items.length === 0) return <p className="text-sm text-zinc-500">{empty}</p>;
  return (
    <div className="divide-y divide-zinc-200 dark:divide-zinc-800">
      {items.map((item) => (
        <NewsCard key={item.id} item={item} />
      ))}
    </div>
  );
}
