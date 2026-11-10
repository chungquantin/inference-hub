import type { Metadata } from "next";
import { FilterChip, Pagination, SearchForm, listingHref, parsePage } from "@/components/listing";
import { NewsList } from "@/components/news";
import { PageHeader } from "@/components/ui/primitives";
import { api } from "@/lib/api";

export const metadata: Metadata = {
  title: "News",
  description: "News from model providers, inference clouds, GPU providers and open-source projects.",
  alternates: { canonical: "/news" },
};

const PAGE_SIZE = 30;

type Props = { searchParams: Promise<{ q?: string; organization?: string; page?: string }> };

export default async function NewsPage({ searchParams }: Props) {
  const { q, organization, page } = await searchParams;
  const pageNumber = parsePage(page);
  const { data, error } = await api.GET("/v1/news", {
    params: {
      query: {
        q: q || undefined,
        organization: organization || undefined,
        limit: PAGE_SIZE,
        offset: (pageNumber - 1) * PAGE_SIZE,
      },
    },
  });
  if (error || !data) throw new Error("Could not load news");
  const current = { q, organization };

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        label="News"
        title="What the market shipped"
        lead="From company blogs, research labs and the press, tagged with the companies they are about."
      />
      <SearchForm defaultValue={q} placeholder="Search news" hidden={{ organization }} />
      {organization && (
        <div className="flex gap-2 text-sm">
          <FilterChip href={listingHref("/news", { q })} active>
            About: {organization} ✕
          </FilterChip>
        </div>
      )}
      <NewsList
        items={data.items}
        empty={q || organization ? "No news matches. Remove a filter to see more." : "No news yet."}
      />
      <Pagination
        page={pageNumber}
        lastPage={Math.max(1, Math.ceil(data.total / PAGE_SIZE))}
        hrefFor={(p) => listingHref("/news", current, { page: String(p) })}
      />
    </div>
  );
}
