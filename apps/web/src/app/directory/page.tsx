import type { Metadata } from "next";
import { DirectoryApp } from "@/components/directory/directory-app";
import type { DirectoryData, Row, Tab } from "@/components/directory/types";
import { api } from "@/lib/api";
import { modelLogoUrl } from "@/lib/catalog";

export const revalidate = 300; // seconds; must be a literal for Next.js

export const metadata: Metadata = {
  title: "Directory",
  description:
    "Search every GPU, model and company in the inference market. Filter in plain English, sort, and compare prices in one table.",
  alternates: { canonical: "/directory" },
};

const TABS: Tab[] = ["gpus", "models", "orgs"];

export default async function DirectoryPage({
  searchParams,
}: {
  searchParams: Promise<{ tab?: string; q?: string }>;
}) {
  const params = await searchParams;
  const tab = TABS.includes(params.tab as Tab) ? (params.tab as Tab) : "gpus";

  const [gpus, models, orgs] = await Promise.all([
    api.GET("/v1/market/gpus"),
    api.GET("/v1/market/models", { params: { query: { min_providers: 2, sort: "price", limit: 500 } } }),
    api.GET("/v1/organizations", { params: { query: { limit: 100 } } }),
  ]);
  if (gpus.error || models.error || orgs.error) throw new Error("Could not load the directory");

  const data: DirectoryData = {
    gpus: (gpus.data ?? [])
      .filter((g) => g.min_per_gpu_hour > 0)
      .map(
        (g): Row => ({
          id: g.gpu_type,
          name: g.gpu_type,
          sub: `${g.providers} clouds`,
          href: `/market/gpus/${encodeURIComponent(g.gpu_type)}`,
          clouds: g.providers,
          offers: g.listings,
          lo: g.min_per_gpu_hour,
          mid: g.median_per_gpu_hour,
          hi: g.max_per_gpu_hour,
          at: g.cheapest.name,
          gap: g.max_per_gpu_hour / g.min_per_gpu_hour,
        }),
      ),
    models: (models.data?.items ?? [])
      .filter((m) => m.min_blended_per_mtok > 0)
      .map(
        (m): Row => ({
          id: m.model_key,
          name: m.model_key,
          sub: m.maker ?? "",
          href: `/market/models/${encodeURIComponent(m.model_key)}`,
          logo: modelLogoUrl(m.model_key, m.maker),
          maker: m.maker ?? "",
          providers: m.providers,
          lo: m.min_blended_per_mtok,
          mid: m.median_blended_per_mtok,
          inPrice: m.min_input_per_mtok,
          outPrice: m.min_output_per_mtok,
          ctx: m.max_context_window,
          caps: m.capabilities.filter((c) => !c.startsWith("in:") && !c.startsWith("out:")),
          at: m.cheapest.name,
          gap: m.median_blended_per_mtok / m.min_blended_per_mtok,
        }),
      ),
    orgs: (orgs.data?.items ?? []).map(
      (o): Row => ({
        id: o.slug,
        name: o.name,
        sub: o.tagline ?? "",
        href: `/organizations/${o.slug}`,
        logo: o.logo_url,
        kinds: o.kinds,
        tagline: o.tagline ?? "",
        web: (o.website ?? "").replace(/^https?:\/\/(www\.)?/, "").replace(/\/.*/, ""),
        country: o.hq_country ?? "",
        founded: o.founded_year,
        status: o.status,
      }),
    ),
  };

  return (
    <div data-wide>
      <DirectoryApp data={data} initialTab={tab} initialQuery={params.q ?? ""} />
    </div>
  );
}
