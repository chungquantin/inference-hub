import type { Metadata } from "next";
import { DataTable } from "@/components/data-table";
import { FilterChip, Pagination, SearchForm, listingHref, parsePage } from "@/components/listing";
import { MEDIA_KINDS, RowLink, formatUnitPrice, modelHref, spread } from "@/components/market";
import { ProviderLink } from "@/components/price-cells";
import { PageHeader, Price } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatCount } from "@/lib/format";

export const metadata: Metadata = {
  title: "Image, video, speech and embedding model prices",
  description:
    "Image, video, text-to-speech, transcription and embedding models across providers, priced per image, per second, per character and per token.",
  alternates: { canonical: "/market/media" },
};

type Props = { searchParams: Promise<{ kind?: string; q?: string; min?: string; page?: string }> };

const PAGE_SIZE = 100;
type Kind = (typeof MEDIA_KINDS)[number]["kind"];

export default async function MarketMediaPage({ searchParams }: Props) {
  const params = await searchParams;
  const kind = MEDIA_KINDS.find((k) => k.kind === params.kind)?.kind as Kind | undefined;
  const min = params.min === "2" ? 2 : 1;
  const pageNumber = parsePage(params.page);
  const { data, error } = await api.GET("/v1/market/media", {
    params: {
      query: {
        kind,
        q: params.q || undefined,
        min_providers: min,
        limit: PAGE_SIZE,
        offset: (pageNumber - 1) * PAGE_SIZE,
      },
    },
  });
  if (error || !data) throw new Error("Could not load the media index");
  const current = { kind, q: params.q, min: params.min };
  const href = (o: Record<string, string | undefined>) =>
    listingHref("/market/media", current, { page: undefined, ...o });

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        label="Market / Media"
        title="Image, video, speech and embeddings"
        lead="Models that are not priced by chat tokens, compared in their own unit: per image, per second of video or audio, per character of speech, and per million input tokens for embeddings."
      />
      <SearchForm
        action="/market/media"
        defaultValue={params.q}
        placeholder="Search models, such as flux or whisper"
        hidden={{ kind, min: params.min }}
      />
      <nav className="flex flex-wrap gap-2 text-sm" aria-label="Kind">
        <FilterChip href={href({ kind: undefined })} active={!kind}>
          All kinds
        </FilterChip>
        {MEDIA_KINDS.map((k) => (
          <FilterChip key={k.kind} href={href({ kind: k.kind })} active={kind === k.kind}>
            {k.label}
          </FilterChip>
        ))}
        <span className="mx-1 w-px self-stretch bg-line" aria-hidden />
        <FilterChip href={href({ min: min === 2 ? undefined : "2" })} active={min === 2}>
          Sold by 2+ providers
        </FilterChip>
      </nav>
      {data.items.length === 0 ? (
        <p className="font-mono text-[13px] text-ink-3">No model matches. Remove a filter to see more.</p>
      ) : (
        <DataTable
          caption={`${formatCount(data.total)} models`}
          headers={["Model", "Kind", "Providers", "Lowest", "Median", "Spread", "Cheapest at"]}
          numeric={[2, 3, 4, 5]}
          rows={data.items.map((row) => ({
            key: `${row.kind}:${row.model_key}`,
            cells: [
              <RowLink key="m" href={modelHref(row.model_key)}>
                {row.name}
              </RowLink>,
              <span key="k" className="text-ink-3">
                {MEDIA_KINDS.find((k) => k.kind === row.kind)?.label ?? row.kind}
              </span>,
              <Price key="n">{row.providers}</Price>,
              <Price key="lo" strong>
                {formatUnitPrice(row.min_price, row.unit)}
              </Price>,
              <Price key="md">{formatUnitPrice(row.median_price, row.unit)}</Price>,
              <Price key="sp">{row.providers > 1 ? spread(row.min_price, row.median_price) : "—"}</Price>,
              <ProviderLink
                key="c"
                slug={row.cheapest.slug}
                name={row.cheapest.name}
                logoUrl={row.cheapest.logo_url}
              />,
            ],
          }))}
        />
      )}
      <Pagination
        page={pageNumber}
        lastPage={Math.max(1, Math.ceil(data.total / PAGE_SIZE))}
        hrefFor={(p) => listingHref("/market/media", current, { page: String(p) })}
      />
    </div>
  );
}
