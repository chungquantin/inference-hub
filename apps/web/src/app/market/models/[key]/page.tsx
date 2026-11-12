import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { MarketListings } from "@/components/market-detail";
import { HyperLink } from "@/components/hyper-link";
import { Logo } from "@/components/ui/logo";
import { PageHeader, Stat } from "@/components/ui/primitives";
import { creatorName, modelLogoUrl } from "@/lib/catalog";
import { api } from "@/lib/api";
import { formatUnitPrice } from "@/components/market";
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
  const maker = data.maker ?? null;
  const logo = <Logo name={creatorName(maker) ?? data.key} src={modelLogoUrl(data.key, maker)} size="lg" />;
  const byline = maker && (
    <span className="flex items-center gap-2 text-[15px]">
      by <HyperLink type="creator" maker={maker} />
    </span>
  );
  const media = data.listings.filter((l) => l.unit);
  const chat = data.listings.filter((l) => !l.unit);
  if (chat.length === 0) {
    const prices = media.map((l) => l.price_per_unit ?? 0).sort((a, b) => a - b);
    const unit = media[0]?.unit;
    return (
      <div className="flex flex-col gap-6">
        <PageHeader
          crumbs={[{ href: "/market", label: "Market" }, { href: "/market/models", label: "Models" }, { label: data.key }]}
          logo={logo}
          title={data.name}
          lead={
          <>
            {byline}
            <span className="mt-1 block">Each provider&apos;s listing of this model, priced in its own unit, cheapest first.</span>
          </>
        }
        />
        <section className="grid grid-cols-2 border border-line bg-wash sm:grid-cols-3">
          <Stat value={formatUnitPrice(prices[0], unit)} label="lowest" />
          <Stat value={formatUnitPrice(prices[Math.floor(prices.length / 2)], unit)} label="median" />
          <Stat value={new Set(media.map((l) => l.variant ?? l.provider.slug)).size} label="sellers" />
        </section>
        <MarketListings listings={media} kind="media" />
      </div>
    );
  }
  const priced = chat.filter((l) => (l.blended_per_mtok ?? 0) > 0);
  const blends = priced.map((l) => l.blended_per_mtok ?? 0).sort((a, b) => a - b);
  const sellers = new Set(chat.map((l) => l.variant ?? l.provider.slug)).size;
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        crumbs={[{ href: "/market", label: "Market" }, { href: "/market/models", label: "Models" }, { label: data.key }]}
        logo={logo}
        title={data.name}
        lead={
          <>
            {byline}
            <span className="mt-1 block">
              Each provider&apos;s listing of this model, matched by id. Routers such as Hugging Face show the provider behind
              each price. Blend is 3 input tokens per output token.
            </span>
          </>
        }
      />
      <section className="grid grid-cols-2 border border-line bg-wash sm:grid-cols-4">
        <Stat value={formatPrice(blends[0])} label="lowest blend / 1M" />
        <Stat value={formatPrice(blends[Math.floor(blends.length / 2)])} label="median blend / 1M" />
        <Stat value={sellers} label="sellers" />
        <Stat value={chat.length} label="listings" />
      </section>
      <MarketListings listings={chat} kind="model" />
      {media.length > 0 && <MarketListings listings={media} kind="media" />}
    </div>
  );
}
