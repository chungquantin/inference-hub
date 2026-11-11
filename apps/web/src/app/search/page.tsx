import type { Metadata } from "next";
import Link from "next/link";
import { FilterChip, SearchForm, listingHref } from "@/components/listing";
import { Badge, PageHeader } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatCount, formatDate } from "@/lib/format";

export const metadata: Metadata = {
  title: "Search",
  robots: { index: false },
};

const KINDS = [
  { kind: "model", label: "Models", all: "/market/models" },
  { kind: "media", label: "Image, video, speech", all: "/market/media" },
  { kind: "gpu", label: "GPUs", all: "/market/gpus" },
  { kind: "organization", label: "Organizations", all: "/organizations" },
  { kind: "product", label: "Products", all: "/products" },
  { kind: "news", label: "News", all: "/news" },
] as const;

type Props = { searchParams: Promise<{ q?: string; kind?: string }> };

export default async function SearchPage({ searchParams }: Props) {
  const params = await searchParams;
  const q = (params.q ?? "").trim();
  const kind = KINDS.find((k) => k.kind === params.kind)?.kind;
  const { data } =
    q.length >= 2
      ? await api.GET("/v1/search", { params: { query: { q, kinds: kind, per_kind: kind ? 50 : 8 } } })
      : { data: undefined };
  const counts = new Map(data?.counts.map((c) => [c.kind, c.count]) ?? []);
  const total = [...counts.values()].reduce((a, b) => a + b, 0);

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        label="Search"
        title={q ? `Results for “${q}”` : "Search the hub"}
        lead={
          q && data
            ? `${formatCount(total)} matches across models, GPUs, providers, products and news.`
            : "Find models, GPU types, providers, products and news. Words match in order, so “deepseek v3” finds DeepSeek-V3.1."
        }
      />
      <SearchForm action="/search" defaultValue={q} placeholder="Search everything" hidden={{ kind }} />
      {q && (
        <nav className="flex flex-wrap gap-2 text-sm" aria-label="Kind">
          <FilterChip href={listingHref("/search", { q }, {})} active={!kind}>
            All · {formatCount(total)}
          </FilterChip>
          {KINDS.filter((k) => counts.get(k.kind)).map((k) => (
            <FilterChip key={k.kind} href={listingHref("/search", { q, kind: k.kind }, {})} active={kind === k.kind}>
              {k.label} · {formatCount(counts.get(k.kind) ?? 0)}
            </FilterChip>
          ))}
        </nav>
      )}
      {q.length > 0 && q.length < 2 && <p className="font-mono text-[13px] text-ink-3">Type at least two characters.</p>}
      {data && data.hits.length === 0 && (
        <p className="font-mono text-[13px] text-ink-3">Nothing matches. Try fewer words or another spelling.</p>
      )}
      {data &&
        KINDS.filter((k) => data.hits.some((h) => h.kind === k.kind)).map((k) => {
          const hits = data.hits.filter((h) => h.kind === k.kind);
          const more = (counts.get(k.kind) ?? 0) - hits.length;
          return (
            <section key={k.kind} className="flex flex-col gap-2">
              <div className="flex items-baseline justify-between">
                <h2 className="font-display text-[22px] tracking-[-0.02em]">
                  {k.label}
                  <span className="ml-2 font-mono text-sm text-ink-3">{formatCount(counts.get(k.kind) ?? 0)}</span>
                </h2>
                {more > 0 && (
                  <Link
                    href={kind ? `${k.all}?q=${encodeURIComponent(q)}` : listingHref("/search", { q, kind: k.kind }, {})}
                    className="font-mono text-[13px] text-s-7 underline"
                  >
                    {kind ? `Open in ${k.label.toLowerCase()} index` : `See ${formatCount(more)} more`}
                  </Link>
                )}
              </div>
              <ul className="divide-y divide-line border border-line bg-wash">
                {hits.map((h) => {
                  const external = h.kind === "news";
                  const body = (
                    <>
                      {h.logo_url ? (
                        // eslint-disable-next-line @next/next/no-img-element
                        <img src={h.logo_url} alt="" width={24} height={24} className="size-6 shrink-0 object-contain" />
                      ) : (
                        <span aria-hidden className="size-6 shrink-0 bg-field" />
                      )}
                      <span className="flex min-w-0 flex-1 flex-col">
                        <span className="truncate text-[15px] text-ink group-hover:text-s-7">{h.title}</span>
                        {h.subtitle && <span className="truncate text-[13px] text-ink-3">{h.subtitle}</span>}
                      </span>
                      {h.date && <span className="font-mono text-[12px] text-ink-3">{formatDate(h.date)}</span>}
                      {h.score >= 3 && <Badge tone="brand">exact</Badge>}
                    </>
                  );
                  return (
                    <li key={`${h.kind}:${h.href}`}>
                      {external ? (
                        <a href={h.href} rel="noopener" target="_blank" className="group flex items-center gap-3 px-4 py-3 hover:bg-paper">
                          {body}
                        </a>
                      ) : (
                        <Link href={h.href} className="group flex items-center gap-3 px-4 py-3 hover:bg-paper">
                          {body}
                        </Link>
                      )}
                    </li>
                  );
                })}
              </ul>
            </section>
          );
        })}
    </div>
  );
}
