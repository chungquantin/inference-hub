import type { Metadata } from "next";
import { RankingsDashboard } from "@/components/rankings/dashboard";
import { ago } from "@/components/market";
import { LiveDot } from "@/components/ui/pixel";
import { PageHeader } from "@/components/ui/primitives";
import { api } from "@/lib/api";

export const revalidate = 300; // seconds; must be a literal for Next.js

export const metadata: Metadata = {
  title: "Inference market rankings",
  description:
    "Rank models, GPUs and providers by price, spread and how often they are the cheapest: where shopping around saves the most.",
  alternates: { canonical: "/analytics/rankings" },
};

const TABS = ["models", "gpus", "providers"] as const;

export default async function RankingsPage({ searchParams }: { searchParams: Promise<{ tab?: string }> }) {
  const { tab } = await searchParams;
  const { data, error } = await api.GET("/v1/market/analytics");
  if (error || !data) throw new Error("Could not load the rankings");
  const initialTab = TABS.find((t) => t === tab) ?? "models";
  return (
    <div className="flex flex-col gap-8">
      <PageHeader
        crumbs={[{ href: "/analytics", label: "Analytics" }, { label: "Rankings" }]}
        title="Rankings"
        lead="Which models, GPUs and providers are cheapest, and where the price gap between sellers is widest. Computed from every live price our adapters collect: per million tokens at a 3:1 input-to-output blend, and per GPU-hour. Free promotional listings are left out."
      >
        {data.refreshed_at && (
          <span className="flex items-center gap-2 font-mono text-[13px] text-ink-3">
            <LiveDot /> computed {ago(data.refreshed_at, new Date().toISOString())}
          </span>
        )}
      </PageHeader>
      <RankingsDashboard data={data} initialTab={initialTab} />
    </div>
  );
}
