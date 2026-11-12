import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { MarketListings } from "@/components/market-detail";
import { PageHeader, Stat } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatPrice } from "@/lib/format";

export const revalidate = 300; // seconds; must be a literal for Next.js

type Props = { params: Promise<{ type: string }> };

async function load(type: string) {
  const { data, response } = await api.GET("/v1/market/gpus/{gpu_type}", {
    params: { path: { gpu_type: decodeURIComponent(type) } },
  });
  if (response.status === 404) notFound();
  if (!data) throw new Error("Could not load the GPU prices");
  return data;
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const data = await load((await params).type);
  return {
    title: `${data.name} prices across clouds`,
    description: `On-demand ${data.name} prices per GPU-hour at ${new Set(data.listings.map((l) => l.provider.slug)).size} clouds.`,
    alternates: { canonical: `/analytics/gpus/${encodeURIComponent(data.key)}` },
  };
}

export default async function MarketGpuPage({ params }: Props) {
  const data = await load((await params).type);
  const prices = data.listings.map((l) => l.price_per_gpu_hour ?? 0).sort((a, b) => a - b);
  const median = prices[Math.floor(prices.length / 2)];
  const clouds = new Set(data.listings.map((l) => l.provider.slug)).size;
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        crumbs={[{ href: "/analytics", label: "Analytics" }, { href: "/analytics/gpus", label: "GPUs" }, { label: data.name }]}
        title={data.name}
        lead="Every configuration we see, per GPU-hour on demand. Marketplace rows are medians of host offers; Shadeform rows name the cloud behind them."
      />
      <section className="grid grid-cols-2 border border-line bg-wash sm:grid-cols-4">
        <Stat value={formatPrice(prices[0])} label="lowest per GPU-hour" />
        <Stat value={formatPrice(median)} label="median" />
        <Stat value={clouds} label="clouds" />
        <Stat value={data.listings.length} label="configurations" />
      </section>
      <MarketListings listings={data.listings} kind="gpu" />
    </div>
  );
}
