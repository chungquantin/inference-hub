import { FilterChip, Pagination, SearchForm, listingHref } from "@/components/listing";
import { MEDIA_KINDS, formatUnitPrice, gpuHref, modelHref } from "@/components/market";
import { Capabilities } from "@/components/capabilities";
import { HyperLink } from "@/components/hyper-link";
import { ModelName } from "@/components/model-name";
import { SmartTable, type Column } from "@/components/table/smart-table";
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
    above >= 10
      ? `${Math.round(above)}× low`
      : above >= 2
        ? `${above.toFixed(1)}× low`
        : `+${Math.max(1, Math.round((above - 1) * 100))}% vs low`;
  return (
    <span className="text-ink-3 tabular">
      #{o.rank} of {o.market_providers} · {gap}
    </span>
  );
}

export type OfferingsParams = { kind?: string; q?: string; sort?: string; dir?: string; cheapest?: string };

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
  params,
  page,
  pageSize,
}: {
  name: string;
  slug: string;
  provider: Provider | undefined;
  offerings: Offerings;
  params: OfferingsParams;
  page: number;
  pageSize: number;
}) {
  const base = `/organizations/${slug}`;
  const current = { ...params };
  const href = (o: Record<string, string | undefined>) =>
    `${listingHref(base, current, { page: undefined, ...o })}#offerings`;
  const all = offerings.kinds.reduce((n, k) => n + k.count, 0);
  const max = Math.max(...offerings.items.map((o) => o.market_median ?? o.price ?? 0), 0);
  const kind = params.kind;
  const shownKind = kind ?? (offerings.kinds.length === 1 ? offerings.kinds[0].kind : undefined);
  const priceHeader = shownKind === "model" ? "Price / 1M" : shownKind === "gpu" ? "Price / GPU-h" : "Price";

  const columns: Column<Offering>[] = [
    {
      id: "item",
      header: "Item",
      sort: "name",
      cell: (o) =>
        o.kind === "gpu" ? (
          <HyperLink
            type="gpu"
            gpuType={o.name}
            href={itemHref(o)}
            className="inline-flex items-center gap-2.5 font-sans text-[14px] text-ink underline decoration-line-strong hover:text-s-7"
          />
        ) : (
          <ModelName
            id={o.item_key}
            href={itemHref(o)}
            maker={o.maker}
            subtitle={
              [
                o.name !== o.item_key ? o.name : null,
                o.kind === "model" && o.context_window ? `${formatTokens(o.context_window)} ctx` : null,
                o.variants > 1 ? `${o.variants} variants` : null,
              ]
                .filter(Boolean)
                .join(" · ") || undefined
            }
          />
        ),
      csv: (o) => o.name,
    },
    {
      id: "caps",
      header: "Capabilities",
      optional: true,
      title: "What this provider says the model can do",
      cell: (o) => (o.kind === "gpu" ? null : <Capabilities caps={o.capabilities} />),
      csv: (o) => o.capabilities.join(" "),
    },
    {
      id: "kind",
      header: "Kind",
      optional: true,
      cell: (o) => <span className="text-ink-3">{KIND_LABELS[o.kind] ?? o.kind}</span>,
      csv: (o) => o.kind,
    },
    {
      id: "price",
      header: priceHeader,
      numeric: true,
      sort: "price",
      title: "Chat models: blend of 3 input tokens per output token",
      cell: (o) => (
        <span className="flex flex-col items-end">
          <Price strong>{priceOf(o, o.price)}</Price>
          {o.kind === "model" && o.input_per_mtok != null && (
            <span className="text-[12px] text-ink-3 tabular">
              {formatPrice(o.input_per_mtok)} in · {formatPrice(o.output_per_mtok)} out
            </span>
          )}
        </span>
      ),
      csv: (o) => o.price,
    },
    {
      id: "bar",
      header: "",
      className: "hidden w-[100px] lg:table-cell",
      cell: (o) =>
        o.price != null && (
          <PixelBar value={o.price} max={max} cells={14} tone={o.rank === 1 ? "lead" : "muted"} label={priceOf(o, o.price)} />
        ),
    },
    { id: "low", header: "Market low", numeric: true, optional: true, cell: (o) => <Price>{priceOf(o, o.market_min)}</Price>, csv: (o) => o.market_min },
    { id: "median", header: "Median", numeric: true, optional: true, cell: (o) => <Price>{priceOf(o, o.market_median)}</Price>, csv: (o) => o.market_median },
    {
      id: "standing",
      header: "Standing",
      sort: "gap",
      title: "Rank among every seller, and how far above the lowest price",
      cell: (o) => <Standing o={o} />,
      csv: (o) => (o.rank ? `${o.rank}/${o.market_providers}` : ""),
    },
    {
      id: "sellers",
      header: "Sellers",
      numeric: true,
      optional: true,
      sort: "providers",
      sortDir: "desc",
      cell: (o) => <Price>{o.market_providers ?? "—"}</Price>,
      csv: (o) => o.market_providers,
    },
    {
      id: "source",
      header: "Source",
      optional: true,
      cell: (o) => (
        <span className="flex flex-wrap gap-1">
          {o.sources.includes("adapter") &&
            (o.source_url ? (
              <a href={o.source_url} rel="noopener" target="_blank" className="text-ink-3 underline hover:text-s-7" title="Collected from the provider's catalog">
                live
              </a>
            ) : (
              <span className="text-ink-3">live</span>
            ))}
          {o.sources.includes("curated") && <Badge tone="brand">curated</Badge>}
        </span>
      ),
      csv: (o) => o.sources.join("+"),
    },
  ];

  return (
    <section id="offerings" className="flex scroll-mt-20 flex-col gap-4">
      <SectionHeader title={`What ${name} sells`} count={all} />
      {provider && (
        <div className="grid grid-cols-2 border border-line bg-wash sm:grid-cols-4">
          <Stat value={formatCount(provider.models)} label="chat models" href={href({ kind: "model", q: undefined })} />
          <Stat value={formatCount(provider.media_models)} label="image, video, speech, embedding" />
          <Stat value={formatCount(provider.gpu_configurations)} label="GPU configurations" href={href({ kind: "gpu", q: undefined })} />
          <Stat
            value={formatCount(provider.cheapest_count)}
            label="cheapest in the market on"
            href={href({ cheapest: "1", sort: "providers" })}
          />
        </div>
      )}
      <div className="flex flex-wrap items-center gap-2">
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
        <span className="mx-1 hidden h-6 w-px bg-line sm:block" aria-hidden />
        <FilterChip href={href({ cheapest: params.cheapest ? undefined : "1" })} active={!!params.cheapest}>
          Cheapest in market
        </FilterChip>
      </div>
      {all > pageSize && (
        <SearchForm
          action={`${base}#offerings`}
          defaultValue={params.q}
          placeholder={`Search what ${name} sells`}
          hidden={{ kind, sort: params.sort, dir: params.dir, cheapest: params.cheapest }}
        />
      )}
      <SmartTable
        id={`offerings-${slug}`}
        caption={`${formatCount(offerings.total)} ${kind ? (KIND_LABELS[kind] ?? kind).toLowerCase() : "items"}${
          params.cheapest ? " where it is cheapest" : ""
        } · market low and median cover every seller we track`}
        columns={columns}
        rows={offerings.items}
        rowKey={(o) => `${o.kind}:${o.item_key}`}
        sort={{ sort: params.sort ?? "providers", dir: params.dir === "asc" || params.dir === "desc" ? params.dir : undefined }}
        sortHref={(sort, dir) => href({ sort, dir })}
        csvName={`${slug}-offerings`}
        empty="Nothing matches. Clear the search or a filter to see everything."
      />
      <Pagination
        page={page}
        lastPage={Math.max(1, Math.ceil(offerings.total / pageSize))}
        hrefFor={(p) => `${listingHref(base, current, { page: String(p) })}#offerings`}
      />
    </section>
  );
}
