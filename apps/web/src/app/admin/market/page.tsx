import type { Metadata } from "next";
import Link from "next/link";
import { DataTable } from "@/components/data-table";
import { FilterChip, Pagination, listingHref, parsePage } from "@/components/listing";
import { adminApi, requireAdmin } from "@/lib/admin";
import { formatDate, formatPrice } from "@/lib/format";

export const metadata: Metadata = { title: "Market adapters", robots: { index: false } };

const PAGE_SIZE = 50;
const KINDS = ["model", "gpu"] as const;

type Props = { searchParams: Promise<{ adapter?: string; kind?: string; page?: string }> };

export default async function AdminMarketPage({ searchParams }: Props) {
  await requireAdmin();
  const params = await searchParams;
  const kind = KINDS.find((k) => k === params.kind);
  const pageNumber = parsePage(params.page);
  const client = adminApi();

  const [adapters, unmapped] = await Promise.all([
    client.GET("/v1/admin/adapters"),
    client.GET("/v1/admin/listings/unmapped", {
      params: {
        query: {
          adapter: params.adapter || undefined,
          kind,
          limit: PAGE_SIZE,
          offset: (pageNumber - 1) * PAGE_SIZE,
        },
      },
    }),
  ]);
  if (!adapters.data || !unmapped.data) throw new Error("Could not load market data");

  const current = { adapter: params.adapter, kind };
  const href = (overrides: Record<string, string | undefined>) =>
    listingHref("/admin/market", current, { page: undefined, ...overrides });

  return (
    <div className="flex flex-col gap-10">
      <header className="flex items-baseline justify-between">
        <h1 className="text-2xl font-semibold">Market adapters</h1>
        <Link href="/admin" className="text-sm text-ink-2 hover:text-ink">
          ← Review queue
        </Link>
      </header>

      <section className="flex flex-col gap-3">
        <h2 className="text-xl font-semibold">Adapters</h2>
        <DataTable
          headers={["Adapter", "Listings", "Mapped", "Last success", "Status"]}
          numeric={[1, 2]}
          rows={adapters.data.map((a) => ({
            key: a.adapter,
            cells: [
              <Link key="a" href={href({ adapter: a.adapter })} className="font-medium hover:underline">
                {a.adapter}
              </Link>,
              a.listings_seen,
              a.mapped,
              a.last_success_at ? formatDate(a.last_success_at) : "never",
              a.consecutive_failures > 0 ? (
                <span key="s" className="text-up" title={a.last_error ?? undefined}>
                  failing ×{a.consecutive_failures}: {a.last_error}
                </span>
              ) : (
                "ok"
              ),
            ],
          }))}
        />
      </section>

      <section className="flex flex-col gap-3">
        <h2 className="text-xl font-semibold">Unmapped listings</h2>
        <p className="max-w-[65ch] text-sm text-ink-2">
          Models and GPUs the adapters see that no product is mapped to. To track one, add its id
          under the product&apos;s <code className="font-mono">listings:</code> in the data, such as{" "}
          <code className="font-mono">listings: {"{ openrouter: [openai/gpt-oss-120b] }"}</code>,
          and run the seed.
        </p>
        <nav className="flex flex-wrap gap-2 text-sm" aria-label="Filter listings">
          <FilterChip href={href({ kind: undefined, adapter: undefined })} active={!kind && !params.adapter}>
            All
          </FilterChip>
          {KINDS.map((k) => (
            <FilterChip key={k} href={href({ kind: k })} active={k === kind}>
              {k === "model" ? "Models" : "GPUs"}
            </FilterChip>
          ))}
          {params.adapter && (
            <FilterChip href={href({ adapter: undefined })} active>
              Adapter: {params.adapter} ✕
            </FilterChip>
          )}
        </nav>
        <p className="font-mono text-[13px] text-ink-3">{unmapped.data.total} unmapped</p>
        <DataTable
          headers={["Adapter", "Id", "Name", "Price", "First seen"]}
          rows={unmapped.data.items.map((l) => ({
            key: `${l.adapter}:${l.external_id}`,
            cells: [
              l.adapter,
              <code key="i" className="font-mono text-xs">
                {l.external_id}
              </code>,
              <span key="n">
                {l.name}
                {l.maker && <span className="ml-1 text-xs text-ink-3">{l.maker}</span>}
              </span>,
              l.kind === "gpu"
                ? `${formatPrice(l.price_per_hour)}/h${l.gpu_type ? ` · ${l.gpu_type}` : ""}`
                : `${formatPrice(l.price_input_per_mtok)} in · ${formatPrice(l.price_output_per_mtok)} out`,
              formatDate(l.first_seen_at),
            ],
          }))}
        />
        <Pagination
          page={pageNumber}
          lastPage={Math.max(1, Math.ceil(unmapped.data.total / PAGE_SIZE))}
          hrefFor={(p) => listingHref("/admin/market", current, { page: String(p) })}
        />
      </section>
    </div>
  );
}
