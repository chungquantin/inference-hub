import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { MarketListings } from "@/components/market-detail";
import { PageHeader, Stat } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatPrice } from "@/lib/format";

export const revalidate = 300; // seconds; must be a literal for Next.js

type Props = { params: Promise<{ key: string }> };

async function load(key: string) {
  const { data, response } = await api.GET("/v1/market/models/{key}", {
    params: { path: { key: decodeURIComponent(key) } },
  });
  if (response.status === 404) notFound();
  if (!data) throw new Error("Could not load the model prices");
  return data;
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const data = await load((await params).key);
  return {
    title: `${data.name} API prices across providers`,
    description: `${data.name} input and output token prices at ${data.listings.length} providers and gateways.`,
    alternates: { canonical: `/market/models/${encodeURIComponent(data.key)}` },
  };
}

export default async function MarketModelPage({ params }: Props) {
  const data = await load((await params).key);
  const priced = data.listings.filter((l) => (l.blended_per_mtok ?? 0) > 0);
  const blends = priced.map((l) => l.blended_per_mtok ?? 0).sort((a, b) => a - b);
  const sellers = new Set(data.listings.map((l) => l.variant ?? l.provider.slug)).size;
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        label={`Market / Models / ${data.key}`}
        title={data.name}
        lead="Each provider's listing of this model, matched by id. Routers such as Hugging Face show the provider behind each price. Blend is 3 input tokens per output token."
      />
      <section className="grid grid-cols-2 border border-line bg-wash sm:grid-cols-4">
        <Stat value={formatPrice(blends[0])} label="lowest blend / 1M" />
        <Stat value={formatPrice(blends[Math.floor(blends.length / 2)])} label="median blend / 1M" />
        <Stat value={sellers} label="sellers" />
        <Stat value={data.listings.length} label="listings" />
      </section>
      <MarketListings listings={data.listings} kind="model" />
    </div>
  );
}
