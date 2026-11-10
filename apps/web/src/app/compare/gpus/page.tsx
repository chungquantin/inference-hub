import type { Metadata } from "next";
import Link from "next/link";
import { DataTable } from "@/components/data-table";
import { FilterChip, listingHref } from "@/components/listing";
import { CheckedOn, ProviderLink } from "@/components/price-cells";
import { api } from "@/lib/api";
import { formatPrice } from "@/lib/format";

export const metadata: Metadata = {
  title: "Compare GPU cloud prices",
  description: "Hourly and per-GPU prices of GPU instances across clouds.",
  alternates: { canonical: "/compare/gpus" },
};

type Props = { searchParams: Promise<{ gpu?: string; provider?: string }> };

export default async function CompareGpusPage({ searchParams }: Props) {
  const params = await searchParams;
  const { data, error } = await api.GET("/v1/compare/gpus", {
    params: { query: { gpu: params.gpu || undefined, provider: params.provider || undefined } },
  });
  if (error || !data) throw new Error("Could not load GPU prices");

  const current = { gpu: params.gpu, provider: params.provider };
  const href = (overrides: Record<string, string | undefined>) =>
    listingHref("/compare/gpus", current, overrides);

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-col gap-3">
        <h1 className="text-3xl font-semibold tracking-tight">GPU cloud prices</h1>
        <p className="max-w-3xl text-zinc-600 dark:text-zinc-400">
          On-demand prices of GPU instances, cheapest per GPU-hour first. Reserved and spot prices
          are usually lower.
        </p>
      </header>

      <nav className="flex flex-wrap items-center gap-2 text-sm" aria-label="Filter by GPU">
        <FilterChip href={href({ gpu: undefined })} active={!params.gpu}>
          All GPUs
        </FilterChip>
        {data.gpu_types.map((gpu) => (
          <FilterChip key={gpu} href={href({ gpu })} active={params.gpu?.toLowerCase() === gpu.toLowerCase()}>
            {gpu}
          </FilterChip>
        ))}
        {params.provider && (
          <FilterChip href={href({ provider: undefined })} active>
            Provider: {params.provider} ✕
          </FilterChip>
        )}
      </nav>

      {data.prices.length === 0 ? (
        <p className="text-sm text-zinc-500">No prices match these filters.</p>
      ) : (
        <DataTable
          caption={`${data.prices.length} prices`}
          headers={["GPU", "Provider", "GPUs", "Per GPU-hour", "Per hour", "Region", "Checked"]}
          numeric={[2, 3, 4]}
          rows={data.prices.map((row) => ({
            key: `${row.provider.slug}:${row.gpu_type}:${row.gpu_count}:${row.region ?? ""}`,
            cells: [
              <Link key="g" href={`/products/${row.product_slug}`} className="font-medium hover:underline">
                {row.gpu_type}
              </Link>,
              <ProviderLink key="p" slug={row.provider.slug} name={row.provider.name} />,
              row.gpu_count,
              <span key="g1" className="font-medium">
                {formatPrice(row.price_per_gpu_hour, row.currency)}
              </span>,
              formatPrice(row.price_per_hour, row.currency),
              row.region ?? "—",
              <CheckedOn key="c" date={row.observed_at} source={row.source_url} />,
            ],
          }))}
        />
      )}
      <p className="text-xs text-zinc-500">
        Prices are curated by hand and can be out of date: check the provider before you buy.
      </p>
    </div>
  );
}
