import type { Metadata } from "next";
import Link from "next/link";
import { DataTable } from "@/components/data-table";
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
      <header className="flex flex-col gap-3">
        <h1 className="text-3xl font-semibold tracking-tight">Open-source projects</h1>
        <p className="max-w-3xl text-zinc-600 dark:text-zinc-400">
          Inference engines and runtimes by GitHub stars. Stars over 30 days appear once the hub
          has 30 days of history. Archived projects are listed last.
        </p>
      </header>
      {data.length === 0 ? (
        <p className="text-sm text-zinc-500">No repository stats yet. The worker syncs them every few hours.</p>
      ) : (
        <DataTable
          headers={["Project", "Stars", "30 days", "Forks", "License", "Latest release", "Last push"]}
          numeric={[1, 2, 3]}
          rows={data.map((row) => ({
            key: row.product_slug,
            cells: [
              <div key="p">
                <Link href={`/products/${row.product_slug}`} className="font-medium hover:underline">
                  {row.product_name}
                </Link>
                <a
                  href={`https://github.com/${row.repo}`}
                  rel="noopener"
                  target="_blank"
                  className="ml-2 font-mono text-xs text-zinc-500 hover:underline"
                >
                  {row.repo}
                </a>
                {row.archived && <span className="ml-2 text-xs text-amber-600">archived</span>}
              </div>,
              formatCount(row.stars),
              row.stars_30d != null ? `+${formatCount(row.stars_30d)}` : "—",
              formatCount(row.forks),
              row.license ?? "—",
              row.latest_release ? (
                <span key="r">
                  <span className="font-mono text-xs">{row.latest_release}</span>
                  {row.released_at && <span className="ml-1 text-zinc-500">{formatDate(row.released_at)}</span>}
                </span>
              ) : (
                "—"
              ),
              formatDate(row.pushed_at),
            ],
          }))}
        />
      )}
    </div>
  );
}
