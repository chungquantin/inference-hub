import type { Metadata } from "next";
import type { OrgKind } from "@inference-hub/api-client";
import { FilterChip, Pagination, SearchForm, listingHref, parsePage } from "@/components/listing";
import { OrganizationGrid } from "@/components/organization-card";
import { api } from "@/lib/api";
import { ORG_KIND_LABELS, isOrgKind } from "@/lib/labels";

export const metadata: Metadata = {
  title: "Directory",
  description: "Companies, projects and investors of the inference market.",
  alternates: { canonical: "/organizations" },
};

const PAGE_SIZE = 24;

type Props = {
  searchParams: Promise<{ q?: string; kind?: string; page?: string }>;
};

export default async function OrganizationsPage({ searchParams }: Props) {
  const { q, kind, page } = await searchParams;
  const activeKind = isOrgKind(kind) ? kind : undefined;
  const pageNumber = parsePage(page);

  const { data, error } = await api.GET("/v1/organizations", {
    params: {
      query: {
        q: q || undefined,
        kind: activeKind,
        limit: PAGE_SIZE,
        offset: (pageNumber - 1) * PAGE_SIZE,
      },
    },
  });
  if (error || !data) throw new Error("Could not load organizations");

  const current = { q, kind: activeKind };
  const href = (overrides: Record<string, string | undefined>) =>
    listingHref("/organizations", current, overrides);

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-col gap-4">
        <h1 className="text-3xl font-semibold tracking-tight">Directory</h1>
        <SearchForm
          defaultValue={q}
          placeholder="Search companies and projects"
          hidden={{ kind: activeKind }}
        />
        <nav className="flex flex-wrap gap-2 text-sm" aria-label="Filter by role">
          <FilterChip href={href({ kind: undefined })} active={!activeKind}>
            All
          </FilterChip>
          {(Object.keys(ORG_KIND_LABELS) as OrgKind[]).map((k) => (
            <FilterChip key={k} href={href({ kind: k })} active={k === activeKind}>
              {ORG_KIND_LABELS[k]}
            </FilterChip>
          ))}
        </nav>
      </header>

      <p className="text-sm text-zinc-500">
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
