import type { Metadata } from "next";
import { DataTable } from "@/components/data-table";
import { FilterChip, SearchForm, listingHref } from "@/components/listing";
import { RowLink, SpreadBar, gpuHref, spread } from "@/components/market";
import { ProviderLink } from "@/components/price-cells";
import { PageHeader, Price } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatPrice } from "@/lib/format";

export const metadata: Metadata = {
  title: "GPU price index",
  description: "Every GPU type across every cloud we track: lowest, median and highest price per GPU-hour.",
  alternates: { canonical: "/market/gpus" },
};

type Props = { searchParams: Promise<{ q?: string; min?: string }> };

export default async function MarketGpusPage({ searchParams }: Props) {
  const params = await searchParams;
  const min = params.min === "2" ? 2 : 1;
  const { data, error } = await api.GET("/v1/market/gpus", {
    params: { query: { q: params.q || undefined, min_providers: min, limit: 500 } },
  });
  if (error || !data) throw new Error("Could not load the GPU index");
  const href = (o: Record<string, string | undefined>) =>
    listingHref("/market/gpus", { q: params.q, min: params.min }, o);

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        label="Market / GPUs"
        title="GPU price index"
        lead="On-demand price per GPU-hour of each GPU type, across clouds, marketplaces and hyperscalers. Marketplace prices (Vast.ai, Akash) are medians of host offers."
      />
      <SearchForm action="/market/gpus" defaultValue={params.q} placeholder="Search GPUs, such as H100" hidden={{ min: params.min }} />
      <nav className="flex flex-wrap gap-2 text-sm" aria-label="Filter">
        <FilterChip href={href({ min: undefined })} active={min === 1}>
          All
        </FilterChip>
        <FilterChip href={href({ min: "2" })} active={min === 2}>
          Sold by 2+ clouds
        </FilterChip>
      </nav>
      {data.length === 0 ? (
        <p className="font-mono text-[13px] text-ink-3">No GPU matches. Try a shorter search.</p>
      ) : (
        <DataTable
          caption={`${data.length} GPU types`}
          headers={["GPU", "Clouds", "Configs", "Lowest", "Median", "Highest", "Spread", "", "Cheapest at"]}
          numeric={[1, 2, 3, 4, 5, 6]}
          rows={data.map((row) => ({
            key: row.gpu_type,
            cells: [
              <RowLink key="g" href={gpuHref(row.gpu_type)}>
                {row.gpu_type}
              </RowLink>,
              <Price key="n">{row.providers}</Price>,
              <Price key="l">{row.listings}</Price>,
              <Price key="lo" strong>
                {formatPrice(row.min_per_gpu_hour)}
              </Price>,
              <Price key="md">{formatPrice(row.median_per_gpu_hour)}</Price>,
              <Price key="hi">{formatPrice(row.max_per_gpu_hour)}</Price>,
              <Price key="sp">{spread(row.min_per_gpu_hour, row.max_per_gpu_hour)}</Price>,
              <span key="bar" className="hidden w-[110px] md:block">
                <SpreadBar min={row.min_per_gpu_hour} median={row.median_per_gpu_hour} max={row.max_per_gpu_hour} unit="per GPU-hour" />
              </span>,
              <ProviderLink key="c" slug={row.cheapest.slug} name={row.cheapest.name} logoUrl={row.cheapest.logo_url} />,
            ],
          }))}
        />
      )}
    </div>
  );
}
