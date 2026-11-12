import type { Metadata } from "next";
import { DirectoryApp } from "@/components/directory/directory-app";
import type { DirectoryData, Row, Tab } from "@/components/directory/types";
import { api } from "@/lib/api";
import { modelLogoUrl } from "@/lib/catalog";
import { gpuProducer } from "@/lib/gpu";

export const revalidate = 300; // seconds; must be a literal for Next.js

export const metadata: Metadata = {
  title: "Directory",
  description:
    "Search every GPU, model and company in the inference market. Filter in plain English, sort, and compare prices in one table.",
  alternates: { canonical: "/directory" },
};

const TABS: Tab[] = ["gpus", "models", "orgs"];

/** The largest page the API serves (market lists allow 500, other lists 100). */
const PAGE = 100;

/**
 * Every item of a paged list, fetched page by page: the API caps a page, so a
 * single request silently drops the rest. Returns null if any page fails.
 */
async function allPages<T>(
  get: (offset: number, limit: number) => Promise<{ data?: { items: T[]; total: number }; error?: unknown }>,
): Promise<T[] | null> {
  const items: T[] = [];
  for (let offset = 0; ; offset += PAGE) {
    const { data, error } = await get(offset, PAGE);
    if (error || !data) return null;
    items.push(...data.items);
    if (data.items.length === 0 || items.length >= data.total) return items;
  }
}

export default async function DirectoryPage({
  searchParams,
}: {
  searchParams: Promise<{ tab?: string; q?: string }>;
}) {
  const params = await searchParams;
  const tab = TABS.includes(params.tab as Tab) ? (params.tab as Tab) : "gpus";

  const [gpus, models, orgs, everyModel] = await Promise.all([
    api.GET("/v1/market/gpus"),
    allPages((offset, limit) =>
      api.GET("/v1/market/models", { params: { query: { min_providers: 2, sort: "price", limit, offset } } }),
    ),
    allPages((offset, limit) => api.GET("/v1/organizations", { params: { query: { limit, offset } } })),
    // Only the count: the full list loads in the browser when the Models tab opens.
    api.GET("/v1/market/models", { params: { query: { min_providers: 1, limit: 1 } } }),
  ]);
  if (gpus.error || !models || !orgs || !everyModel.data) throw new Error("Could not load the directory");

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
          atOrg: { slug: g.cheapest.slug, name: g.cheapest.name, logo: g.cheapest.logo_url ?? null },
          logo: gpuProducer(g.gpu_type)?.logo ?? null,
          producer: gpuProducer(g.gpu_type)?.name ?? "",
          gap: g.max_per_gpu_hour / g.min_per_gpu_hour,
        }),
      ),
    models: models
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
          atOrg: { slug: m.cheapest.slug, name: m.cheapest.name, logo: m.cheapest.logo_url ?? null },
          gap: m.median_blended_per_mtok / m.min_blended_per_mtok,
        }),
      ),
    orgs: orgs.map(
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
    modelTotal: everyModel.data.total,
  };

  return (
    <div data-wide>
      <DirectoryApp data={data} initialTab={tab} initialQuery={params.q ?? ""} />
    </div>
  );
}
