import type { Metadata } from "next";
import Link from "next/link";
import { DataTable } from "@/components/data-table";
import { HyperLink } from "@/components/hyper-link";
import { PixelBar } from "@/components/ui/pixel";
import { PageHeader, Price } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatCount } from "@/lib/format";

export const revalidate = 300; // seconds; must be a literal for Next.js

export const metadata: Metadata = {
  title: "Inference providers by coverage",
  description: "Every model API provider and GPU cloud we track, by catalog size and how often they are the cheapest.",
  alternates: { canonical: "/market/providers" },
};

export default async function MarketProvidersPage() {
  const { data, error } = await api.GET("/v1/market/providers");
  if (error || !data) throw new Error("Could not load providers");
  const maxWins = Math.max(...data.map((p) => p.cheapest_count), 0);
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        crumbs={[{ href: "/market", label: "Market" }, { label: "Providers" }]}
        title="Providers"
        lead="Catalog size, and how many contested models or GPU types each provider sells cheapest. Chat models are distinct models; listings count every variant the provider lists (regions, :free, batch). Only items two or more providers sell count as contested."
      />
      <DataTable
        caption={`${data.length} providers`}
        headers={["#", "Provider", "Chat models", "Listings", "Media models", "GPU configs", "Cheapest on", ""]}
        numeric={[0, 2, 3, 4, 5, 6]}
        rows={data.map((p, i) => ({
          key: p.organization.slug,
          cells: [
            <Price key="r">{i + 1}</Price>,
            <HyperLink type="org" key="p" slug={p.organization.slug} name={p.organization.name} logoUrl={p.organization.logo_url} />,
            p.models ? (
              <Link
                key="m"
                href={`/market/models?provider=${p.organization.slug}`}
                className="font-mono text-ink underline decoration-line-strong tabular hover:text-s-7"
              >
                {formatCount(p.models)}
              </Link>
            ) : (
              <Price key="m">—</Price>
            ),
            <Price key="l">{p.model_listings ? formatCount(p.model_listings) : "—"}</Price>,
            <Price key="md">{p.media_models ? formatCount(p.media_models) : "—"}</Price>,
            <Price key="g">{p.gpu_configurations || "—"}</Price>,
            <Price key="w" strong>
              {p.cheapest_count}
            </Price>,
            <span key="bar" className="hidden w-[150px] md:block">
              <PixelBar value={p.cheapest_count} max={maxWins} cells={20} label={`cheapest on ${p.cheapest_count}`} />
            </span>,
          ],
        }))}
      />
    </div>
  );
}
