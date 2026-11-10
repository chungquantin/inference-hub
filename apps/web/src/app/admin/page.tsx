import type { Metadata } from "next";
import Link from "next/link";
import type { NewsStatus } from "@inference-hub/api-client";
import { FilterChip, Pagination, listingHref, parsePage } from "@/components/listing";
import { NewsCard } from "@/components/news";
import { buttonClass } from "@/components/ui/primitives";
import { adminApi, requireAdmin } from "@/lib/admin";
import { formatDate } from "@/lib/format";
import { logout, review, setOrganizations } from "./actions";

export const metadata: Metadata = { title: "Review queue", robots: { index: false } };

const PAGE_SIZE = 50;
const STATUSES: NewsStatus[] = ["pending", "approved", "rejected"];

type Props = { searchParams: Promise<{ status?: string; source?: string; page?: string }> };

export default async function AdminPage({ searchParams }: Props) {
  await requireAdmin();
  const { status: rawStatus, source, page } = await searchParams;
  const status = STATUSES.find((s) => s === rawStatus) ?? "pending";
  const pageNumber = parsePage(page);
  const client = adminApi();

  const [news, sources] = await Promise.all([
    client.GET("/v1/admin/news", {
      params: {
        query: {
          status,
          source: source || undefined,
          limit: PAGE_SIZE,
          offset: (pageNumber - 1) * PAGE_SIZE,
        },
      },
    }),
    client.GET("/v1/admin/sources"),
  ]);
  if (!news.data || !sources.data) throw new Error("Could not load the review queue");

  const current = { status, source };
  const href = (overrides: Record<string, string | undefined>) =>
    listingHref("/admin", current, { page: undefined, ...overrides });
  const otherStatuses = STATUSES.filter((s) => s !== status);

  return (
    <div className="flex flex-col gap-10">
      <header className="flex items-baseline justify-between">
        <h1 className="text-2xl font-semibold">Review queue</h1>
        <div className="flex items-baseline gap-4 text-sm">
          <Link href="/admin/market" className="text-ink-2 hover:text-ink">
            Market adapters
          </Link>
          <form action={logout}>
            <button type="submit" className="text-ink-2 hover:text-ink">
              Sign out
            </button>
          </form>
        </div>
      </header>

      <section className="flex flex-col gap-4">
        <nav className="flex flex-wrap gap-2 text-sm" aria-label="Filter by status">
          {STATUSES.map((s) => (
            <FilterChip key={s} href={href({ status: s })} active={s === status}>
              {s[0].toUpperCase() + s.slice(1)}
            </FilterChip>
          ))}
          {source && (
            <FilterChip href={href({ source: undefined })} active>
              Source: {source} ✕
            </FilterChip>
          )}
        </nav>
        <p className="font-mono text-[13px] text-ink-3">
          {news.data.total} {status} {news.data.total === 1 ? "item" : "items"}
        </p>

        {news.data.items.length > 0 && (
          <form action={review} id="review" className="flex flex-wrap items-center gap-2 text-sm">
            <span className="text-ink-3">Checked items:</span>
            {otherStatuses.map((s) => (
              <button
                key={s}
                type="submit"
                name="status"
                value={s}
                className={buttonClass("secondary")}
              >
                {s === "approved" ? "Approve" : s === "rejected" ? "Reject" : "Back to queue"}
              </button>
            ))}
          </form>
        )}

        <div className="divide-y divide-line border border-line bg-wash">
          {news.data.items.map((item) => (
            <div key={item.id} className="flex gap-3">
              <input
                type="checkbox"
                name="id"
                value={item.id}
                form="review"
                aria-label={`Select ${item.title}`}
                className="mt-5 size-4 shrink-0"
              />
              <div className="min-w-0 flex-1">
                <NewsCard item={item}>
                  <form action={setOrganizations} className="flex gap-2 pt-1 text-xs">
                    <input type="hidden" name="id" value={item.id} />
                    <input
                      name="organizations"
                      defaultValue={item.organizations.map((o) => o.slug).join(", ")}
                      aria-label="Organization slugs, comma separated"
                      placeholder="organization slugs, comma separated"
                      className="h-8 w-full max-w-md border border-line bg-wash px-2 font-mono text-[13px] text-ink"
                    />
                    <button type="submit" className="text-ink-2 hover:text-ink">
                      Save tags
                    </button>
                  </form>
                </NewsCard>
              </div>
            </div>
          ))}
        </div>
        <Pagination
          page={pageNumber}
          lastPage={Math.max(1, Math.ceil(news.data.total / PAGE_SIZE))}
          hrefFor={(p) => href({ page: String(p) })}
        />
      </section>

      <section className="flex flex-col gap-3">
        <h2 className="text-xl font-semibold">Sources</h2>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-line text-left text-ink-3">
                {["Source", "Owner", "Items", "Pending", "Last success", "Status"].map((h) => (
                  <th key={h} scope="col" className="py-2 pr-4 font-normal">
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {sources.data.map((s) => (
                <tr key={s.id}>
                  <td className="py-2 pr-4">
                    <Link href={href({ source: s.slug, status: "pending" })} className="hover:underline">
                      {s.name}
                    </Link>
                    {!s.enabled && <span className="ml-2 text-xs text-ink-3">disabled</span>}
                  </td>
                  <td className="py-2 pr-4">{s.organization_name ?? "—"}</td>
                  <td className="py-2 pr-4 tabular-nums">{s.item_count}</td>
                  <td className="py-2 pr-4 tabular-nums">{s.pending_count}</td>
                  <td className="py-2 pr-4">{s.last_success_at ? formatDate(s.last_success_at) : "never"}</td>
                  <td className="py-2 pr-4">
                    {s.consecutive_failures > 0 ? (
                      <span className="text-up" title={s.last_error ?? undefined}>
                        failing ×{s.consecutive_failures}
                      </span>
                    ) : s.last_run_at ? (
                      "ok"
                    ) : (
                      "not run yet"
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}
