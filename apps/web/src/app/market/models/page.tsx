import type { Metadata } from "next";
import { DataTable } from "@/components/data-table";
import { FilterChip, SearchForm, listingHref } from "@/components/listing";
import { RowLink, modelHref } from "@/components/market";
import { ProviderLink } from "@/components/price-cells";
import { PageHeader, Price } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatPrice, formatTokens } from "@/lib/format";

export const metadata: Metadata = {
  title: "Model price index",
  description: "Every model across every provider we track: the cheapest and median price per million tokens.",
  alternates: { canonical: "/market/models" },
};

type Props = { searchParams: Promise<{ q?: string; min?: string }> };

const MINS = ["1", "2", "5"] as const;

export default async function MarketModelsPage({ searchParams }: Props) {
  const params = await searchParams;
  // Without a search, show contested models first: thousands are sold by one provider only.
  const min = MINS.includes(params.min as (typeof MINS)[number]) ? Number(params.min) : params.q ? 1 : 2;
  const { data, error } = await api.GET("/v1/market/models", {
    params: { query: { q: params.q || undefined, min_providers: min, limit: 300 } },
  });
  if (error || !data) throw new Error("Could not load the model index");
  const href = (o: Record<string, string | undefined>) =>
    listingHref("/market/models", { q: params.q, min: params.min }, o);

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        label="Market / Models"
        title="Model price index"
        lead="The same model across providers, routers and gateways, matched by its id. Blend is 3 input tokens per output token; free promotional listings are left out."
      />
      <SearchForm action="/market/models" defaultValue={params.q} placeholder="Search models, such as deepseek or llama" hidden={{ min: params.min }} />
      <nav className="flex flex-wrap gap-2 text-sm" aria-label="Minimum providers">
        {MINS.map((m) => (
          <FilterChip key={m} href={href({ min: m })} active={min === Number(m)}>
            {m === "1" ? "All" : `${m}+ providers`}
          </FilterChip>
        ))}
      </nav>
      {data.length === 0 ? (
        <p className="font-mono text-[13px] text-ink-3">No model matches. Try a shorter search or fewer providers.</p>
      ) : (
        <DataTable
          caption={`${data.length} models${data.length === 300 ? " (first 300)" : ""}`}
          headers={["Model", "Maker", "Providers", "Lowest in", "Lowest out", "Lowest blend", "Median blend", "Context", "Cheapest at"]}
          numeric={[2, 3, 4, 5, 6, 7]}
          rows={data.map((row) => ({
            key: row.model_key,
            cells: [
              <RowLink key="m" href={modelHref(row.model_key)}>
                {row.model_key}
              </RowLink>,
              <span key="mk" className="text-ink-3">
                {row.maker ?? "—"}
              </span>,
              <Price key="n">{row.providers}</Price>,
              <Price key="i">{formatPrice(row.min_input_per_mtok)}</Price>,
              <Price key="o">{formatPrice(row.min_output_per_mtok)}</Price>,
              <Price key="b" strong>
                {formatPrice(row.min_blended_per_mtok)}
              </Price>,
              <Price key="mb">{formatPrice(row.median_blended_per_mtok)}</Price>,
              <Price key="c">{formatTokens(row.max_context_window)}</Price>,
              <ProviderLink key="p" slug={row.cheapest.slug} name={row.cheapest.name} logoUrl={row.cheapest.logo_url} />,
            ],
          }))}
        />
      )}
    </div>
  );
}
