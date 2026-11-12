import type { Metadata } from "next";
import { listingHref } from "@/components/listing";
import { SpreadBar, spread } from "@/components/market";
import { HyperLink } from "@/components/hyper-link";
import { gpuProducer } from "@/lib/gpu";
import { FilterBar } from "@/components/table/filter-bar";
import { SmartTable, type Column } from "@/components/table/smart-table";
import { PageHeader, Price } from "@/components/ui/primitives";
import type { GpuIndexRow } from "@inference-hub/api-client";
import { api } from "@/lib/api";
import { formatCount, formatPrice } from "@/lib/format";

export const metadata: Metadata = {
  title: "GPU price index",
  description: "Every GPU type across every cloud we track: lowest, median and highest price per GPU-hour.",
  alternates: { canonical: "/analytics/gpus" },
};

type Params = { q?: string; min?: string; provider?: string; max_price?: string; sort?: string; dir?: string };
type Props = { searchParams: Promise<Params> };

const num = (v?: string) => {
  const n = Number(v);
  return v && Number.isFinite(n) && n >= 0 ? n : undefined;
};

export default async function MarketGpusPage({ searchParams }: Props) {
  const params = await searchParams;
  const dir = params.dir === "asc" || params.dir === "desc" ? params.dir : undefined;
  const [{ data, error }, providers] = await Promise.all([
    api.GET("/v1/market/gpus", {
      params: {
        query: {
          q: params.q || undefined,
          provider: params.provider || undefined,
          min_providers: num(params.min) ?? 1,
          max_price: num(params.max_price),
          sort: params.sort || undefined,
          dir,
          limit: 500,
        },
      },
    }),
    api.GET("/v1/market/providers"),
  ]);
  if (error || !data) throw new Error("Could not load the GPU index");
  const values = { q: params.q, provider: params.provider, min: params.min, max_price: params.max_price };
  const current = { ...values, sort: params.sort, dir: params.dir };

  const columns: Column<GpuIndexRow>[] = [
    { id: "gpu", header: "GPU", sort: "name", cell: (r) => <HyperLink type="gpu" gpuType={r.gpu_type} className="inline-flex items-center gap-2 text-ink underline decoration-line-strong hover:text-s-7" />, csv: (r) => r.gpu_type },
    { id: "producer", header: "Made by", optional: true, cell: (r) => <HyperLink type="producer" gpuType={r.gpu_type} />, csv: (r) => gpuProducer(r.gpu_type)?.name ?? "" },
    { id: "clouds", header: "Clouds", numeric: true, sort: "providers", sortDir: "desc", cell: (r) => <Price>{r.providers}</Price>, csv: (r) => r.providers },
    { id: "configs", header: "Configs", numeric: true, optional: true, sort: "listings", sortDir: "desc", cell: (r) => <Price>{r.listings}</Price>, csv: (r) => r.listings },
    { id: "low", header: "Lowest", numeric: true, sort: "price", cell: (r) => <Price strong>{formatPrice(r.min_per_gpu_hour)}</Price>, csv: (r) => r.min_per_gpu_hour },
    { id: "median", header: "Median", numeric: true, sort: "median", cell: (r) => <Price>{formatPrice(r.median_per_gpu_hour)}</Price>, csv: (r) => r.median_per_gpu_hour },
    { id: "high", header: "Highest", numeric: true, optional: true, sort: "max", cell: (r) => <Price>{formatPrice(r.max_per_gpu_hour)}</Price>, csv: (r) => r.max_per_gpu_hour },
    { id: "spread", header: "Spread", numeric: true, optional: true, title: "Highest price over lowest", cell: (r) => <Price>{spread(r.min_per_gpu_hour, r.max_per_gpu_hour)}</Price> },
    {
      id: "bar",
      header: "",
      className: "hidden lg:table-cell w-[110px]",
      cell: (r) => <SpreadBar min={r.min_per_gpu_hour} median={r.median_per_gpu_hour} max={r.max_per_gpu_hour} unit="per GPU-hour" />,
    },
    { id: "cheapest", header: "Cheapest at", optional: true, cell: (r) => <HyperLink type="org" slug={r.cheapest.slug} name={r.cheapest.name} logoUrl={r.cheapest.logo_url} />, csv: (r) => r.cheapest.name },
  ];

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        crumbs={[{ href: "/analytics", label: "Analytics" }, { label: "GPUs" }]}
        title="GPU price index"
        lead="On-demand price per GPU-hour of each GPU type, across clouds, marketplaces and hyperscalers. Marketplace prices (Vast.ai, Akash) are medians of host offers."
      />
      <FilterBar
        action="/analytics/gpus"
        values={values}
        keep={{ sort: params.sort, dir: params.dir }}
        filters={[
          { type: "search", name: "q", label: "GPU", placeholder: "h100, b200, 4090…" },
          {
            type: "select",
            name: "provider",
            label: "Cloud",
            options: (providers.data ?? [])
              .filter((p) => p.gpu_configurations > 0)
              .map((p) => ({ value: p.organization.slug, label: `${p.organization.name} (${p.gpu_configurations})` })),
          },
          { type: "number", name: "max_price", label: "Max / GPU-hour", placeholder: "2.00", prefix: "$" },
          { type: "select", name: "min", label: "Clouds", options: [{ value: "2", label: "2+" }, { value: "5", label: "5+" }] },
        ]}
        labels={{ q: "GPU", provider: "Cloud", min: "Clouds", max_price: "Max" }}
        describe={{ max_price: (v) => `$${v} / GPU-h`, min: (v) => `${v}+` }}
        presets={[
          { label: "Data-center GPUs", params: { min: "5" } },
          { label: "Under $1 / GPU-hour", params: { max_price: "1", sort: "price" } },
          { label: "H100", params: { q: "h100", sort: "price" } },
          { label: "Blackwell", params: { q: "b200" } },
        ]}
      />
      <SmartTable
        id="market-gpus"
        caption={`${formatCount(data.length)} GPU types`}
        columns={columns}
        rows={data}
        rowKey={(r) => r.gpu_type}
        sort={{ sort: params.sort ?? "providers", dir }}
        sortHref={(sort, d) => listingHref("/analytics/gpus", current, { sort, dir: d })}
        csvName="inference-hub-gpus"
        empty="No GPU matches these filters."
      />
    </div>
  );
}
