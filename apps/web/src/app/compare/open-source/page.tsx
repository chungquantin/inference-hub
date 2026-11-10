import type { Metadata } from "next";
import Link from "next/link";
import { DataTable } from "@/components/data-table";
import { Logo } from "@/components/ui/logo";
import { Badge, PageHeader, Price } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatCount, formatDate } from "@/lib/format";

export const revalidate = 3600; // seconds; must be a literal for Next.js

export const metadata: Metadata = {
  title: "Open-source inference projects",
  description: "Inference engines and runtimes ranked by GitHub stars, with recent momentum.",
  alternates: { canonical: "/compare/open-source" },
};

export default async function CompareOpenSourcePage() {
  const { data, error } = await api.GET("/v1/compare/open-source");
  if (error || !data) throw new Error("Could not load open-source projects");

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Open-source projects"
        lead="Inference engines and runtimes by GitHub stars. Stars over 30 days appear once the hub has 30 days of history. Archived projects are listed last."
      />
      {data.length === 0 ? (
        <p className="text-sm text-ink-3">
          No repository stats yet. The worker syncs them every few hours.
        </p>
      ) : (
        <DataTable
          headers={["Project", "Stars", "30 days", "Forks", "License", "Latest release", "Last push"]}
          numeric={[1, 2, 3]}
          rows={data.map((row) => ({
            key: row.product_slug,
            cells: [
              <div key="p" className="flex items-center gap-2.5">
                <Logo name={row.organization.name} src={row.organization.logo_url} size="md" />
                <div className="min-w-0">
                  <Link
                    href={`/products/${row.product_slug}`}
                    className="font-medium text-ink hover:text-accent"
                  >
                    {row.product_name}
                  </Link>
                  <div className="flex items-center gap-1.5 text-xs">
                    <a
                      href={`https://github.com/${row.repo}`}
                      rel="noopener"
                      target="_blank"
                      className="font-mono text-ink-3 hover:text-ink"
                    >
                      {row.repo}
                    </a>
                    {row.archived && <Badge tone="amber">archived</Badge>}
                  </div>
                </div>
              </div>,
              <Price key="s" strong>
                {formatCount(row.stars)}
              </Price>,
              <Price key="t">{row.stars_30d != null ? `+${formatCount(row.stars_30d)}` : "—"}</Price>,
              <Price key="f">{formatCount(row.forks)}</Price>,
              <span key="l" className="text-ink-2">
                {row.license ?? "—"}
              </span>,
              row.latest_release ? (
                <span key="r" className="text-ink-2">
                  <span className="font-mono text-xs text-ink">{row.latest_release}</span>
                  {row.released_at && <span className="ml-1.5">{formatDate(row.released_at)}</span>}
                </span>
              ) : (
                <span key="r" className="text-ink-3">
                  —
                </span>
              ),
              <span key="d" className="text-ink-2">
                {formatDate(row.pushed_at)}
              </span>,
            ],
          }))}
        />
      )}
    </div>
  );
}
