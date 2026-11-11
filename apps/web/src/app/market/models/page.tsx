import type { Metadata } from "next";
import { Pagination, listingHref, parsePage } from "@/components/listing";
import { RowLink, modelHref } from "@/components/market";
import { ProviderLink } from "@/components/price-cells";
import { CAPABILITIES, Capabilities, CapabilityLegend } from "@/components/capabilities";
import { FilterBar, type Filter } from "@/components/table/filter-bar";
import { SmartTable, type Column } from "@/components/table/smart-table";
import { PixelBar } from "@/components/ui/pixel";
import { PageHeader, Price } from "@/components/ui/primitives";
import type { ModelIndexRow } from "@inference-hub/api-client";
import { api } from "@/lib/api";
import { formatCount, formatPrice, formatTokens } from "@/lib/format";

export const metadata: Metadata = {
  title: "Model price index",
  description:
    "Every chat model across providers, routers and gateways: lowest and median price per million tokens, context window and who sells it cheapest. Filter by maker, price, context and provider.",
  alternates: { canonical: "/market/models" },
};

type Params = {
  q?: string;
  maker?: string;
  provider?: string;
  min?: string;
  max_price?: string;
  min_context?: string;
  caps?: string;
  sort?: string;
  dir?: string;
  page?: string;
};
type Props = { searchParams: Promise<Params> };

const PAGE_SIZE = 100;
const CONTEXTS = [
  { value: "32000", label: "32K+" },
  { value: "128000", label: "128K+" },
  { value: "200000", label: "200K+" },
  { value: "1000000", label: "1M+" },
];
const num = (v?: string) => {
  const n = Number(v);
  return v && Number.isFinite(n) && n >= 0 ? n : undefined;
};

export default async function MarketModelsPage({ searchParams }: Props) {
  const params = await searchParams;
  const pageNumber = parsePage(params.page);
  const min = num(params.min) ?? 1;
  const dir = params.dir === "asc" || params.dir === "desc" ? params.dir : undefined;
  const [{ data: page, error }, makers, providers] = await Promise.all([
    api.GET("/v1/market/models", {
      params: {
        query: {
          q: params.q || undefined,
          maker: params.maker || undefined,
          provider: params.provider || undefined,
          min_providers: min,
          max_price: num(params.max_price),
          min_context: num(params.min_context),
          caps: params.caps || undefined,
          sort: params.sort || undefined,
          dir,
          limit: PAGE_SIZE,
          offset: (pageNumber - 1) * PAGE_SIZE,
        },
      },
    }),
    api.GET("/v1/market/makers", { params: { query: { min_providers: 2 } } }),
    api.GET("/v1/market/providers"),
  ]);
  if (error || !page) throw new Error("Could not load the model index");

  const filterValues = {
    q: params.q,
    maker: params.maker,
    provider: params.provider,
    min: params.min,
    max_price: params.max_price,
    min_context: params.min_context,
    caps: params.caps,
  };
  const current = { ...filterValues, sort: params.sort, dir: params.dir };
  const sortHref = (sort: string, d: "asc" | "desc") =>
    listingHref("/market/models", current, { sort, dir: d, page: undefined });
  const first = (pageNumber - 1) * PAGE_SIZE + 1;
  const maxBlend = Math.max(...page.items.map((m) => m.median_blended_per_mtok ?? 0), 0);

  const filters: Filter[] = [
    { type: "search", name: "q", label: "Model", placeholder: "deepseek v3, claude, llama 70b…" },
    {
      type: "select",
      name: "maker",
      label: "Maker",
      options: (makers.data ?? []).map((m) => ({ value: m.maker, label: `${m.maker} (${m.models})` })),
    },
    {
      type: "select",
      name: "provider",
      label: "Sold by",
      options: (providers.data ?? [])
        .filter((p) => p.models > 0)
        .map((p) => ({ value: p.organization.slug, label: `${p.organization.name} (${formatCount(p.models)})` })),
    },
    { type: "number", name: "max_price", label: "Max blend / 1M", placeholder: "0.50", prefix: "$" },
    { type: "select", name: "min_context", label: "Context", options: CONTEXTS },
    {
      type: "select",
      name: "caps",
      label: "Can do",
      options: [
        ...CAPABILITIES.map((c) => ({ value: c.id, label: c.label })),
        { value: "tools,json", label: "Tools + structured output" },
        { value: "in:image,tools", label: "Vision + tools" },
        { value: "reasoning,tools", label: "Reasoning + tools" },
      ],
    },
    {
      type: "select",
      name: "min",
      label: "Providers",
      any: "Any",
      options: [
        { value: "2", label: "2+ (comparable)" },
        { value: "5", label: "5+" },
        { value: "10", label: "10+" },
      ],
    },
  ];

  const columns: Column<ModelIndexRow>[] = [
    {
      id: "model",
      header: "Model",
      sort: "name",
      cell: (r) => (
        <span className="flex flex-col">
          <RowLink href={modelHref(r.model_key)}>{r.model_key}</RowLink>
          {r.name !== r.model_key && <span className="max-w-[260px] truncate text-[12px] text-ink-3">{r.name}</span>}
        </span>
      ),
      csv: (r) => r.model_key,
    },
    {
      id: "caps",
      header: "Capabilities",
      title: "Inputs, → outputs, then features. Text in and out is not drawn.",
      cell: (r) => <Capabilities caps={r.capabilities} />,
      csv: (r) => r.capabilities.join(" "),
    },
    { id: "maker", header: "Maker", optional: true, minWidth: "2xl", cell: (r) => <span className="text-ink-3">{r.maker ?? "—"}</span>, csv: (r) => r.maker },
    { id: "providers", header: "Sellers", numeric: true, sort: "providers", sortDir: "desc", cell: (r) => <Price>{r.providers}</Price>, csv: (r) => r.providers },
    { id: "input", header: "In", numeric: true, optional: true, minWidth: "xl", sort: "input", cell: (r) => <Price>{formatPrice(r.min_input_per_mtok)}</Price>, csv: (r) => r.min_input_per_mtok },
    { id: "output", header: "Out", numeric: true, optional: true, minWidth: "xl", sort: "output", cell: (r) => <Price>{formatPrice(r.min_output_per_mtok)}</Price>, csv: (r) => r.min_output_per_mtok },
    {
      id: "blend",
      header: "Blend",
      title: "3 input tokens per output token, per million",
      numeric: true,
      sort: "price",
      cell: (r) => (
        <Price strong>{formatPrice(r.min_blended_per_mtok)}</Price>
      ),
      csv: (r) => r.min_blended_per_mtok,
    },
    {
      id: "bar",
      header: "",
      optional: false,
      className: "hidden 2xl:table-cell w-[100px]",
      cell: (r) =>
        r.min_blended_per_mtok != null && (
          <PixelBar value={r.min_blended_per_mtok} max={maxBlend} cells={14} label={formatPrice(r.min_blended_per_mtok)} />
        ),
    },
    { id: "median", header: "Median", numeric: true, optional: true, minWidth: "lg", sort: "median", cell: (r) => <Price>{formatPrice(r.median_blended_per_mtok)}</Price>, csv: (r) => r.median_blended_per_mtok },
    { id: "context", header: "Context", numeric: true, optional: true, minWidth: "md", sort: "context", sortDir: "desc", cell: (r) => <Price>{formatTokens(r.max_context_window)}</Price>, csv: (r) => r.max_context_window },
    {
      id: "cheapest",
      header: "Cheapest at",
      optional: true,
      cell: (r) => (
        <span className="block max-w-[180px] truncate">
          <ProviderLink slug={r.cheapest.slug} name={r.cheapest.name} logoUrl={r.cheapest.logo_url} />
        </span>
      ),
      csv: (r) => r.cheapest.name,
    },
  ];

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        crumbs={[{ href: "/market", label: "Market" }, { label: "Models" }]}
        title="Model price index"
        lead="Every chat model we see, matched across providers by its id, with what it can do and its lowest and median price per 1M tokens. Blend is 3 input tokens per output token; free promotional listings are left out of prices."
      />
      <FilterBar
        action="/market/models"
        filters={filters}
        values={filterValues}
        keep={{ sort: params.sort, dir: params.dir }}
        labels={{ q: "Model", maker: "Maker", provider: "Sold by", min: "Providers", max_price: "Max", min_context: "Context", caps: "Can do" }}
        describe={{
          max_price: (v) => `$${v} / 1M`,
          min_context: (v) => `${formatTokens(Number(v))}+`,
          min: (v) => `${v}+`,
          caps: (v) =>
            v
              .split(",")
              .map((c) => CAPABILITIES.find((x) => x.id === c)?.label ?? c)
              .join(" + "),
        }}
        presets={[
          { label: "Comparable (2+ providers)", params: { min: "2" } },
          { label: "Under $0.10 / 1M", params: { max_price: "0.1", min: "2" } },
          { label: "1M+ context", params: { min_context: "1000000", sort: "price" } },
          { label: "Cheapest first", params: { sort: "price", min: "2" } },
          { label: "Vision + tools", params: { caps: "in:image,tools", min: "2" } },
          { label: "Reasoning", params: { caps: "reasoning", min: "2", sort: "price" } },
        ]}
      />
      <CapabilityLegend />
      <SmartTable
        id="market-models"
        caption={
          page.total === 0
            ? "No models"
            : `${formatCount(page.total)} models${page.total > PAGE_SIZE ? ` · ${first}–${first + page.items.length - 1}` : ""}`
        }
        columns={columns}
        rows={page.items}
        rowKey={(r) => r.model_key}
        sort={{ sort: params.sort ?? "providers", dir }}
        sortHref={sortHref}
        csvName="inference-hub-models"
        empty="No model matches these filters. Remove one to see more."
      />
      <Pagination
        page={pageNumber}
        lastPage={Math.max(1, Math.ceil(page.total / PAGE_SIZE))}
        hrefFor={(p) => listingHref("/market/models", current, { page: String(p) })}
      />
    </div>
  );
}
