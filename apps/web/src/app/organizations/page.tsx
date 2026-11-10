import type { Metadata } from "next";
import Link from "next/link";
import type { OrgKind } from "@inference-hub/api-client";
import { OrganizationGrid } from "@/components/organization-card";
import { api } from "@/lib/api";
import { ORG_KIND_LABELS } from "@/lib/labels";

export const metadata: Metadata = { title: "Directory" };

const PAGE_SIZE = 24;

type Props = {
  searchParams: Promise<{ q?: string; kind?: string; page?: string }>;
};

function isOrgKind(value: string | undefined): value is OrgKind {
  return value !== undefined && value in ORG_KIND_LABELS;
}

export default async function OrganizationsPage({ searchParams }: Props) {
  const { q, kind, page } = await searchParams;
  const activeKind = isOrgKind(kind) ? kind : undefined;
  const pageNumber = Math.max(1, Number.parseInt(page ?? "1", 10) || 1);

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

  const lastPage = Math.max(1, Math.ceil(data.total / PAGE_SIZE));
  const hrefFor = (overrides: Record<string, string | undefined>) => {
    const params = new URLSearchParams();
    const merged = { q, kind: activeKind, ...overrides };
    for (const [key, value] of Object.entries(merged)) if (value) params.set(key, value);
    const qs = params.toString();
    return qs ? `/organizations?${qs}` : "/organizations";
  };

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-col gap-4">
        <h1 className="text-3xl font-semibold tracking-tight">Directory</h1>
        <form className="flex max-w-xl gap-2">
          <input
            type="search"
            name="q"
            defaultValue={q}
            placeholder="Search companies and projects"
            className="flex-1 rounded-md border border-zinc-300 bg-transparent px-3 py-2 text-sm outline-none focus:border-zinc-500 dark:border-zinc-700"
          />
          {activeKind && <input type="hidden" name="kind" value={activeKind} />}
          <button
            type="submit"
            className="rounded-md bg-zinc-900 px-4 py-2 text-sm font-medium text-white dark:bg-zinc-100 dark:text-zinc-900"
          >
            Search
          </button>
        </form>
        <nav className="flex flex-wrap gap-2 text-sm" aria-label="Filter by role">
          <FilterChip href={hrefFor({ kind: undefined, page: undefined })} active={!activeKind}>
            All
          </FilterChip>
          {(Object.keys(ORG_KIND_LABELS) as OrgKind[]).map((k) => (
            <FilterChip key={k} href={hrefFor({ kind: k, page: undefined })} active={k === activeKind}>
              {ORG_KIND_LABELS[k]}
            </FilterChip>
          ))}
        </nav>
      </header>

      <p className="text-sm text-zinc-500">
        {data.total} {data.total === 1 ? "result" : "results"}
      </p>
      <OrganizationGrid orgs={data.items} />

      {lastPage > 1 && (
        <nav className="flex items-center justify-between text-sm" aria-label="Pagination">
          {pageNumber > 1 ? (
            <Link href={hrefFor({ page: String(pageNumber - 1) })} className="hover:underline">
              ← Previous
            </Link>
          ) : (
            <span />
          )}
          <span className="text-zinc-500">
            Page {pageNumber} of {lastPage}
          </span>
          {pageNumber < lastPage ? (
            <Link href={hrefFor({ page: String(pageNumber + 1) })} className="hover:underline">
              Next →
            </Link>
          ) : (
            <span />
          )}
        </nav>
      )}
    </div>
  );
}

function FilterChip({
  href,
  active,
  children,
}: {
  href: string;
  active: boolean;
  children: React.ReactNode;
}) {
  return (
    <Link
      href={href}
      aria-current={active ? "page" : undefined}
      className={
        active
          ? "rounded-full bg-zinc-900 px-3 py-1 text-white dark:bg-zinc-100 dark:text-zinc-900"
          : "rounded-full border border-zinc-300 px-3 py-1 text-zinc-700 hover:border-zinc-500 dark:border-zinc-700 dark:text-zinc-300"
      }
    >
      {children}
    </Link>
  );
}
