import type { Metadata } from "next";
import { FilterChip, Pagination, SearchForm, listingHref, parsePage } from "@/components/listing";
import { MEDIA_KINDS, RowLink, UNIT_LABELS, formatUnitPrice, modelHref, spread } from "@/components/market";
import { SmartTable, type Column } from "@/components/table/smart-table";
import type { MediaIndexRow } from "@inference-hub/api-client";
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

  const columns: Column<MediaIndexRow>[] = [
    { id: "model", header: "Model", cell: (r) => <RowLink href={modelHref(r.model_key)}>{r.name}</RowLink>, csv: (r) => r.model_key },
    {
      id: "kind",
      header: "Kind",
      cell: (r) => <span className="text-ink-3">{MEDIA_KINDS.find((k) => k.kind === r.kind)?.label ?? r.kind}</span>,
      csv: (r) => r.kind,
    },
    { id: "maker", header: "Maker", optional: true, cell: (r) => <span className="text-ink-3">{r.maker ?? "—"}</span>, csv: (r) => r.maker },
    { id: "providers", header: "Providers", numeric: true, cell: (r) => <Price>{r.providers}</Price>, csv: (r) => r.providers },
    { id: "low", header: "Lowest", numeric: true, cell: (r) => <Price strong>{formatUnitPrice(r.min_price, r.unit)}</Price>, csv: (r) => r.min_price },
    { id: "median", header: "Median", numeric: true, optional: true, cell: (r) => <Price>{formatUnitPrice(r.median_price, r.unit)}</Price>, csv: (r) => r.median_price },
    { id: "unit", header: "Unit", optional: true, cell: (r) => <span className="text-ink-3">{UNIT_LABELS[r.unit] ?? r.unit}</span>, csv: (r) => r.unit },
    { id: "spread", header: "Spread", numeric: true, optional: true, cell: (r) => <Price>{r.providers > 1 ? spread(r.min_price, r.median_price) : "—"}</Price> },
    { id: "cheapest", header: "Cheapest at", optional: true, cell: (r) => <ProviderLink slug={r.cheapest.slug} name={r.cheapest.name} logoUrl={r.cheapest.logo_url} />, csv: (r) => r.cheapest.name },
  ];

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        crumbs={[{ href: "/market", label: "Market" }, { label: "Image, video, speech" }]}
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
      <SmartTable
        id="market-media"
        caption={`${formatCount(data.total)} models`}
        columns={columns}
        rows={data.items}
        rowKey={(r) => `${r.kind}:${r.model_key}`}
        csvName="inference-hub-media"
        empty="No model matches. Remove a filter to see more."
      />
      <Pagination
        page={pageNumber}
        lastPage={Math.max(1, Math.ceil(data.total / PAGE_SIZE))}
        hrefFor={(p) => listingHref("/market/media", current, { page: String(p) })}
      />
    </div>
  );
}
