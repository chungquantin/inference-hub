import type { Metadata } from "next";
import { Pagination, SearchForm, listingHref, parsePage } from "@/components/listing";
import { NewsList } from "@/components/news";
import { api } from "@/lib/api";

export const metadata: Metadata = {
  title: "News",
  description: "News from model providers, inference clouds, GPU providers and open-source projects.",
  alternates: { canonical: "/news" },
};

const PAGE_SIZE = 30;

type Props = { searchParams: Promise<{ q?: string; page?: string }> };

export default async function NewsPage({ searchParams }: Props) {
  const { q, page } = await searchParams;
  const pageNumber = parsePage(page);
  const { data, error } = await api.GET("/v1/news", {
    params: { query: { q: q || undefined, limit: PAGE_SIZE, offset: (pageNumber - 1) * PAGE_SIZE } },
  });
  if (error || !data) throw new Error("Could not load news");

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-col gap-4">
        <h1 className="text-3xl font-semibold tracking-tight">News</h1>
        <SearchForm defaultValue={q} placeholder="Search news" />
      </header>
      <NewsList items={data.items} empty={q ? "No news matches that search." : "No news yet."} />
      <Pagination
        page={pageNumber}
        lastPage={Math.max(1, Math.ceil(data.total / PAGE_SIZE))}
        hrefFor={(p) => listingHref("/news", { q }, { page: String(p) })}
      />
    </div>
  );
}
