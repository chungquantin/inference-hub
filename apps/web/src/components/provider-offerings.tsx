import Link from "next/link";
import { DataTable } from "@/components/data-table";
import { FilterChip, Pagination, SearchForm, listingHref } from "@/components/listing";
import { MEDIA_KINDS, formatUnitPrice, gpuHref, modelHref } from "@/components/market";
import { PixelBar } from "@/components/ui/pixel";
import { Badge, Price, SectionHeader, Stat } from "@/components/ui/primitives";
import type { components } from "@inference-hub/api-client";
import { formatCount, formatPrice, formatTokens } from "@/lib/format";

type Offerings = components["schemas"]["ProviderOfferings"];
type Offering = components["schemas"]["ProviderOffering"];
type Provider = components["schemas"]["MarketProvider"];

export const KIND_LABELS: Record<string, string> = {
  model: "Chat models",
  gpu: "GPUs",
  ...Object.fromEntries(MEDIA_KINDS.map((k) => [k.kind, k.label])),
};

/** The price of one row, in its kind's unit. */
function priceOf(o: Offering, value: number | null | undefined): string {
  if (o.kind === "model") return formatPrice(value);
  if (o.kind === "gpu") return `${formatPrice(value)} / GPU-h`;
  return formatUnitPrice(value, o.unit);
}

/** The page of the item: the curated product when there is one, else the market's. */
function itemHref(o: Offering): string {
  if (o.product_slug) return `/products/${o.product_slug}`;
  return o.kind === "gpu" ? gpuHref(o.item_key) : modelHref(o.item_key);
}

/** Where this provider's price stands: cheapest, or how far above the market low. */
function Standing({ o }: { o: Offering }) {
  if (o.price == null || o.market_min == null || !o.market_providers || o.market_providers < 2) {
    return <span className="text-ink-3">only seller</span>;
  }
  if (o.rank === 1) return <Badge tone="up">cheapest</Badge>;
  const above = o.market_min > 0 ? o.price / o.market_min : 1;
  // Close prices read better as a percentage than as 1.0×.
  const gap =
    above >= 10 ? `${Math.round(above)}× low` : above >= 2 ? `${above.toFixed(1)}× low` : `+${Math.max(1, Math.round((above - 1) * 100))}% vs low`;
  return (
    <span className="text-ink-3 tabular">
      #{o.rank} of {o.market_providers} · {gap}
    </span>
  );
}

/**
 * Everything one organization sells, in one table: what its catalog lists
 * (collected by the worker) and what the curated data records, merged per
 * item, with the market's lowest price next to the organization's own.
 */
export function ProviderOfferingsSection({
  name,
  slug,
  provider,
  offerings,
  kind,
  q,
  page,
  pageSize,
}: {
  name: string;
  slug: string;
  provider: Provider | undefined;
  offerings: Offerings;
  kind: string | undefined;
  q: string | undefined;
  page: number;
  pageSize: number;
}) {
  const base = `/organizations/${slug}`;
  const current = { kind, q };
  const href = (o: Record<string, string | undefined>) => `${listingHref(base, current, { ...o })}#offerings`;
  const all = offerings.kinds.reduce((n, k) => n + k.count, 0);
  const max = Math.max(...offerings.items.map((o) => o.market_median ?? o.price ?? 0), 0);
  const shownKind = kind ?? (offerings.kinds.length === 1 ? offerings.kinds[0].kind : undefined);
  const priceHeader =
    shownKind === "model"
      ? "Price (blend / 1M)"
      : shownKind === "gpu"
        ? "Price / GPU-hour"
        : "Price";

  return (
    <section id="offerings" className="flex flex-col gap-4">
      <SectionHeader title={`What ${name} sells`} count={all} />
      {provider && (
        <div className="grid grid-cols-2 border border-line bg-wash sm:grid-cols-4">
          <Stat value={formatCount(provider.models)} label="chat models" href={href({ kind: "model", q: undefined })} />
          <Stat value={formatCount(provider.media_models)} label="image, video, speech, embedding" />
          <Stat value={formatCount(provider.gpu_configurations)} label="GPU configurations" />
          <Stat value={formatCount(provider.cheapest_count)} label="cheapest in the market on" />
        </div>
      )}
      <div className="flex flex-col gap-3">
        <nav className="flex flex-wrap gap-2 text-sm" aria-label="Kind">
          {offerings.kinds.length > 1 && (
            <FilterChip href={href({ kind: undefined })} active={!kind}>
              All · {formatCount(all)}
            </FilterChip>
          )}
          {offerings.kinds.map((k) => (
            <FilterChip key={k.kind} href={href({ kind: k.kind })} active={kind === k.kind}>
              {KIND_LABELS[k.kind] ?? k.kind} · {formatCount(k.count)}
            </FilterChip>
          ))}
        </nav>
        {all > pageSize && (
          <SearchForm
            action={`${base}#offerings`}
            defaultValue={q}
            placeholder={`Search what ${name} sells`}
            hidden={{ kind }}
          />
        )}
      </div>
      {offerings.items.length === 0 ? (
        <p className="font-mono text-[13px] text-ink-3">Nothing matches. Clear the search to see everything.</p>
      ) : (
        <DataTable
          caption={
            <span>
              {formatCount(offerings.total)} {kind ? (KIND_LABELS[kind] ?? kind).toLowerCase() : "items"}, the
              most widely sold first. Market low and median cover every provider we track.
            </span>
          }
          headers={["Item", "Kind", priceHeader, "", "Market low", "Market median", "Standing", "Source"]}
          numeric={[2, 4, 5]}
          rows={offerings.items.map((o) => ({
            key: `${o.kind}:${o.item_key}`,
            cells: [
              <span key="n" className="flex flex-col">
                <Link href={itemHref(o)} className="text-ink underline decoration-line-strong hover:text-s-7">
                  {o.name}
                </Link>
                <span className="text-[12px] text-ink-3">
                  {[o.maker, o.kind === "model" && o.context_window ? `${formatTokens(o.context_window)} ctx` : null,
                    o.variants > 1 ? `${o.variants} variants` : null]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </span>,
              <span key="k" className="text-ink-3">
                {KIND_LABELS[o.kind] ?? o.kind}
              </span>,
              <span key="p" className="flex flex-col items-end">
                <Price strong>{priceOf(o, o.price)}</Price>
                {o.kind === "model" && o.input_per_mtok != null && (
                  <span className="text-[12px] text-ink-3 tabular">
                    {formatPrice(o.input_per_mtok)} in · {formatPrice(o.output_per_mtok)} out
                  </span>
                )}
              </span>,
              <span key="bar" className="hidden w-[100px] md:block">
                {o.price != null && (
                  <PixelBar
                    value={o.price}
                    max={max}
                    cells={14}
                    tone={o.rank === 1 ? "lead" : "muted"}
                    label={priceOf(o, o.price)}
                  />
                )}
              </span>,
              <Price key="lo">{priceOf(o, o.market_min)}</Price>,
              <Price key="md">{priceOf(o, o.market_median)}</Price>,
              <Standing key="s" o={o} />,
              <span key="src" className="flex flex-wrap gap-1">
                {o.sources.includes("adapter") && (
                  <span title="Collected from the provider's catalog by the worker">
                    {o.source_url ? (
                      <a href={o.source_url} rel="noopener" target="_blank" className="text-ink-3 underline hover:text-s-7">
                        live
                      </a>
                    ) : (
                      <span className="text-ink-3">live</span>
                    )}
                  </span>
                )}
                {o.sources.includes("curated") && (
                  <Badge tone="brand">curated</Badge>
                )}
              </span>,
            ],
          }))}
        />
      )}
      <Pagination
        page={page}
        lastPage={Math.max(1, Math.ceil(offerings.total / pageSize))}
        hrefFor={(p) => href({ page: String(p) })}
      />
    </section>
  );
}
