import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { FilterChip, Pagination, SearchForm, listingHref, parsePage } from "@/components/listing";
import { OrganizationGrid } from "@/components/organization-card";
import { PageHeader } from "@/components/ui/primitives";
import { api } from "@/lib/api";

export const revalidate = 300; // seconds; must be a literal for Next.js

const PAGE_SIZE = 48;

type Props = {
  params: Promise<{ slug: string }>;
  searchParams: Promise<{ q?: string; page?: string }>;
};

async function loadCategory(slug: string) {
  const { data, response } = await api.GET("/v1/categories/{slug}", {
    params: { path: { slug } },
  });
  if (response.status === 404) notFound();
  if (!data) throw new Error(`Could not load category ${slug}`);
  return data;
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const category = await loadCategory((await params).slug);
  return {
    title: category.name,
    description: category.description ?? undefined,
    alternates: { canonical: `/categories/${category.slug}` },
  };
}

export default async function CategoryPage({ params, searchParams }: Props) {
  const { slug } = await params;
  const { q, page } = await searchParams;
  const pageNumber = parsePage(page);
  const [category, all, orgs] = await Promise.all([
    loadCategory(slug),
    api.GET("/v1/categories"),
    api.GET("/v1/organizations", {
      params: {
        query: { category: slug, q: q || undefined, limit: PAGE_SIZE, offset: (pageNumber - 1) * PAGE_SIZE },
      },
    }),
  ]);
  if (!orgs.data || !all.data) throw new Error(`Could not load organizations for ${slug}`);

  const parent = all.data.find((c) => c.id === category.parent_id);
  const subs = all.data.filter((c) => c.parent_id === category.id);
  const href = (overrides: Record<string, string | undefined>) => listingHref(`/categories/${slug}`, { q }, overrides);

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title={category.name}
        lead={category.description ?? undefined}
        crumbs={[
          { href: "/categories", label: "Categories" },
          ...(parent ? [{ href: `/categories/${parent.slug}`, label: parent.name }] : []),
          { label: category.name },
        ]}
      />
      {subs.length > 0 && (
        <nav className="flex flex-wrap gap-2 text-sm" aria-label="Sub-categories">
          {subs.map((sub) => (
            <FilterChip key={sub.id} href={`/categories/${sub.slug}`} active={false}>
              {sub.name} ({sub.organization_count})
            </FilterChip>
          ))}
        </nav>
      )}
      <SearchForm defaultValue={q} placeholder={`Search ${category.name.toLowerCase()}`} />
      <p className="font-mono text-[13px] text-ink-3">
        {orgs.data.total} {orgs.data.total === 1 ? "result" : "results"}
      </p>
      <OrganizationGrid orgs={orgs.data.items} />
      <Pagination
        page={pageNumber}
        lastPage={Math.max(1, Math.ceil(orgs.data.total / PAGE_SIZE))}
        hrefFor={(p) => href({ page: String(p) })}
      />
    </div>
  );
}
