import type { Metadata } from "next";
import { DataTable } from "@/components/data-table";
import { FilterChip, listingHref } from "@/components/listing";
import { CheckedOn } from "@/components/price-cells";
import { HyperLink } from "@/components/hyper-link";
import { PixelBar } from "@/components/ui/pixel";
import { Badge, PageHeader, Price } from "@/components/ui/primitives";
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
  const rowKey = (i: number) => {
    const row = data.prices[i];
    return `${row.provider.slug}:${row.gpu_type}:${row.gpu_count}:${row.region ?? ""}`;
  };
  // The cheapest row is only meaningful when one GPU type is selected.
  const highlight = params.gpu && data.prices.length > 1 ? rowKey(0) : undefined;
  const max = Math.max(...data.prices.map((p) => p.price_per_gpu_hour), 0);

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        crumbs={[{ href: "/compare", label: "Compare" }, { label: "Tracked GPU clouds" }]}
        title="GPU cloud prices"
        lead="On-demand prices of GPU instances, cheapest per GPU-hour first. Reserved and spot prices are usually lower."
      />

      <nav className="flex flex-wrap items-center gap-2 text-sm" aria-label="Filter by GPU">
        <FilterChip href={href({ gpu: undefined })} active={!params.gpu}>
          All GPUs
        </FilterChip>
        {data.gpu_types.map((gpu) => (
          <FilterChip
            key={gpu}
            href={href({ gpu })}
            active={params.gpu?.toLowerCase() === gpu.toLowerCase()}
          >
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
        <p className="font-mono text-[13px] text-ink-3">No prices match these filters. Remove a filter to see more.</p>
      ) : (
        <DataTable
          caption={`${data.prices.length} prices`}
          headers={["GPU", "Provider", "GPUs", "Per GPU-hour", "", "Per hour", "Region", "Checked"]}
          numeric={[2, 3, 5]}
          highlight={highlight}
          rows={data.prices.map((row, i) => ({
            key: rowKey(i),
            cells: [
              <span key="g" className="flex items-center gap-2 whitespace-nowrap">
                <HyperLink
                  type="gpu"
                  gpuType={row.gpu_type}
                  href={`/products/${row.product_slug}`}
                  className="inline-flex items-center gap-2 text-ink underline decoration-line-strong hover:text-s-7"
                />
                {i === 0 && highlight && <Badge tone="solid">lowest</Badge>}
              </span>,
              <HyperLink
                key="p"
                type="org"
                slug={row.provider.slug}
                name={row.provider.name}
                logoUrl={row.provider.logo_url} />,
              <Price key="n">{row.gpu_count}</Price>,
              <Price key="g1" strong>
                {formatPrice(row.price_per_gpu_hour, row.currency)}
              </Price>,
              <span key="bar" className="hidden w-[150px] md:block">
                <PixelBar
                  value={row.price_per_gpu_hour}
                  max={max}
                  cells={20}
                  tone={i === 0 ? "lead" : "muted"}
                  label={`${formatPrice(row.price_per_gpu_hour, row.currency)} per GPU-hour`}
                />
              </span>,
              <Price key="h">{formatPrice(row.price_per_hour, row.currency)}</Price>,
              <span key="r" className="text-ink-3">
                {row.region ?? "—"}
              </span>,
              <CheckedOn key="c" date={row.observed_at} source={row.source_url} />,
            ],
          }))}
        />
      )}
    </div>
  );
}
