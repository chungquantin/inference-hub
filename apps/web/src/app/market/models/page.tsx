import type { Metadata } from "next";
import { DataTable } from "@/components/data-table";
import { FilterChip, Pagination, SearchForm, listingHref, parsePage } from "@/components/listing";
import { RowLink, modelHref } from "@/components/market";
import { ProviderLink } from "@/components/price-cells";
import { PageHeader, Price } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatCount, formatPrice, formatTokens } from "@/lib/format";

export const metadata: Metadata = {
  title: "Model price index",
  description: "Every model across every provider we track: the cheapest and median price per million tokens.",
  alternates: { canonical: "/market/models" },
};

type Props = { searchParams: Promise<{ q?: string; min?: string; page?: string; provider?: string }> };

const MINS = ["1", "2", "5"] as const;
const PAGE_SIZE = 100;

export default async function MarketModelsPage({ searchParams }: Props) {
  const params = await searchParams;
  // Every model by default; the most widely sold come first.
  const min = MINS.includes(params.min as (typeof MINS)[number]) ? Number(params.min) : 1;
  const pageNumber = parsePage(params.page);
  const { data: page, error } = await api.GET("/v1/market/models", {
    params: {
      query: {
        q: params.q || undefined,
        provider: params.provider || undefined,
        min_providers: min,
        limit: PAGE_SIZE,
        offset: (pageNumber - 1) * PAGE_SIZE,
      },
    },
  });
  if (error || !page) throw new Error("Could not load the model index");
  const data = page.items;
  const current = { q: params.q, min: params.min, provider: params.provider };
  // Filters reset to the first page.
  const href = (o: Record<string, string | undefined>) =>
    listingHref("/market/models", current, { page: undefined, ...o });
  const first = (pageNumber - 1) * PAGE_SIZE + 1;

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        label="Market / Models"
        title="Model price index"
        lead="Every chat model we see, matched across providers by its id: maker and routing prefixes, regions and quantization suffixes are ignored, so anthropic/claude-sonnet-4.5 and bedrock/claude-sonnet-4-5@us-east-1 are one model. Blend is 3 input tokens per output token; free promotional listings are left out of prices."
      />
      <SearchForm action="/market/models" defaultValue={params.q} placeholder="Search models, such as deepseek or llama" hidden={{ min: params.min, provider: params.provider }} />
      <nav className="flex flex-wrap gap-2 text-sm" aria-label="Minimum providers">
        {MINS.map((m) => (
          <FilterChip key={m} href={href({ min: m })} active={min === Number(m)}>
            {m === "1" ? "All models" : `Sold by ${m}+ providers`}
          </FilterChip>
        ))}
        {params.provider && (
          <FilterChip href={href({ provider: undefined })} active>
            Sold by {params.provider} ✕
          </FilterChip>
        )}
      </nav>
      {data.length === 0 ? (
        <p className="font-mono text-[13px] text-ink-3">No model matches. Try a shorter search or fewer providers.</p>
      ) : (
        <DataTable
          caption={`${formatCount(page.total)} models${params.provider ? ` sold by ${params.provider}` : ""}${
            min > 1 ? ` on ${min}+ providers` : ""
          }${
            page.total > PAGE_SIZE ? ` · ${first}–${first + data.length - 1}` : ""
          }`}
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
      <Pagination
        page={pageNumber}
        lastPage={Math.max(1, Math.ceil(page.total / PAGE_SIZE))}
        hrefFor={(p) => listingHref("/market/models", current, { page: String(p) })}
      />
    </div>
  );
}
