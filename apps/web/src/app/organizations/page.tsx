import type { Metadata } from "next";
import type { OrgKind } from "@inference-hub/api-client";
import { FilterChip, Pagination, SearchForm, listingHref, parsePage } from "@/components/listing";
import { OrganizationGrid } from "@/components/organization-card";
import { PageHeader } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { ORG_KIND_LABELS, isOrgKind } from "@/lib/labels";

export const metadata: Metadata = {
  title: "Directory",
  description: "Companies, projects and investors of the inference market.",
  alternates: { canonical: "/organizations" },
};

const PAGE_SIZE = 24;

type Props = {
  searchParams: Promise<{ q?: string; kind?: string; category?: string; page?: string }>;
};

export default async function OrganizationsPage({ searchParams }: Props) {
  const { q, kind, category, page } = await searchParams;
  const activeKind = isOrgKind(kind) ? kind : undefined;
  const pageNumber = parsePage(page);

  const [orgs, categories] = await Promise.all([
    api.GET("/v1/organizations", {
      params: {
        query: {
          q: q || undefined,
          kind: activeKind,
          category: category || undefined,
          limit: PAGE_SIZE,
          offset: (pageNumber - 1) * PAGE_SIZE,
        },
      },
    }),
    api.GET("/v1/categories"),
  ]);
  const { data, error } = orgs;
  if (error || !data || !categories.data) throw new Error("Could not load organizations");
  // Sub-categories, with their parent's name for context.
  const names = new Map(categories.data.map((c) => [c.id, c.name]));
  const subs = categories.data.filter((c) => c.parent_id);
  const activeCategory = categories.data.find((c) => c.slug === category);

  const current = { q, kind: activeKind, category: activeCategory?.slug };
  const href = (overrides: Record<string, string | undefined>) =>
    listingHref("/organizations", current, overrides);

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-col gap-4">
        <PageHeader label="Directory" title="Who is who in inference" lead="Model labs, inference providers, gateways, GPU clouds, hardware makers, open-source projects and the investors behind them." />
        <SearchForm
          defaultValue={q}
          placeholder="Search companies and projects"
          hidden={{ kind: activeKind, category: activeCategory?.slug }}
        />
        <nav className="flex flex-wrap gap-2 text-sm" aria-label="Filter by role">
          <FilterChip href={href({ kind: undefined, page: undefined })} active={!activeKind}>
            All
          </FilterChip>
          {(Object.keys(ORG_KIND_LABELS) as OrgKind[]).map((k) => (
            <FilterChip key={k} href={href({ kind: k, page: undefined })} active={k === activeKind}>
              {ORG_KIND_LABELS[k]}
            </FilterChip>
          ))}
        </nav>
        <nav className="flex flex-wrap gap-2 text-sm" aria-label="Filter by category">
          {activeCategory && (
            <FilterChip href={href({ category: undefined, page: undefined })} active>
              {activeCategory.name} ×
            </FilterChip>
          )}
          {subs
            .filter((c) => c.slug !== activeCategory?.slug)
            .map((c) => (
              <FilterChip key={c.id} href={href({ category: c.slug, page: undefined })} active={false}>
                {c.name}
                <span className="text-ink-3"> · {names.get(c.parent_id ?? "")}</span>
              </FilterChip>
            ))}
        </nav>
      </header>

      <p className="font-mono text-[13px] text-ink-3">
        {data.total} {data.total === 1 ? "result" : "results"}
      </p>
      <OrganizationGrid orgs={data.items} />
      <Pagination
        page={pageNumber}
        lastPage={Math.max(1, Math.ceil(data.total / PAGE_SIZE))}
        hrefFor={(p) => href({ page: String(p) })}
      />
    </div>
  );
}
