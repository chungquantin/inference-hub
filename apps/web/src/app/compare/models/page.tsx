import type { Metadata } from "next";
import Link from "next/link";
import type { ModelPriceSort } from "@inference-hub/api-client";
import { DataTable } from "@/components/data-table";
import { FilterChip, listingHref } from "@/components/listing";
import { CheckedOn, ProviderLink } from "@/components/price-cells";
import { Logo } from "@/components/ui/logo";
import { PixelBar } from "@/components/ui/pixel";
import { Badge, PageHeader, Price } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatPrice, formatTokens } from "@/lib/format";

export const metadata: Metadata = {
  title: "Compare model API prices",
  description:
    "Input, output and blended token prices of language models across inference providers.",
  alternates: { canonical: "/compare/models" },
};

const SORTS: { value: ModelPriceSort; label: string }[] = [
  { value: "blended", label: "Blended price" },
  { value: "input", label: "Input price" },
  { value: "output", label: "Output price" },
  { value: "context_window", label: "Context window" },
];

const CONTEXTS = [
  { value: "32000", label: "32K+" },
  { value: "128000", label: "128K+" },
  { value: "1000000", label: "1M+" },
];

type Props = {
  searchParams: Promise<{ sort?: string; open?: string; context?: string; provider?: string }>;
};

export default async function CompareModelsPage({ searchParams }: Props) {
  const params = await searchParams;
  const sort = SORTS.find((s) => s.value === params.sort)?.value ?? "blended";
  const open = params.open === "1" ? true : undefined;
  const context = CONTEXTS.find((c) => c.value === params.context)?.value;

  const { data, error } = await api.GET("/v1/compare/models", {
    params: {
      query: {
        sort,
        open_weights: open,
        min_context: context ? Number(context) : undefined,
        provider: params.provider || undefined,
      },
    },
  });
  if (error || !data) throw new Error("Could not load model prices");

  const current = {
    sort: sort === "blended" ? undefined : sort,
    open: open ? "1" : undefined,
    context,
    provider: params.provider,
  };
  const href = (overrides: Record<string, string | undefined>) =>
    listingHref("/compare/models", current, overrides);
  const rowKey = (i: number) => `${data[i].product_slug}:${data[i].provider.slug}:${i}`;
  const max = Math.max(...data.map((m) => m.blended_per_mtok ?? 0), 0);

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        crumbs={[{ href: "/compare", label: "Compare" }, { label: "Tracked model APIs" }]}
        title="Model API prices"
        lead="The latest price of each model at each provider, in USD per million tokens. The blended price weighs input and output 3 to 1, a typical chat workload."
      />

      <div className="flex flex-col gap-3 text-sm">
        <nav className="flex flex-wrap items-center gap-2" aria-label="Sort">
          <span className="w-16 font-mono text-[12px] text-ink-3">Sort</span>
          {SORTS.map((s) => (
            <FilterChip
              key={s.value}
              href={href({ sort: s.value === "blended" ? undefined : s.value })}
              active={s.value === sort}
            >
              {s.label}
            </FilterChip>
          ))}
        </nav>
        <nav className="flex flex-wrap items-center gap-2" aria-label="Filter">
          <span className="w-16 font-mono text-[12px] text-ink-3">Show</span>
          <FilterChip href={href({ open: open ? undefined : "1" })} active={Boolean(open)}>
            Open weights
          </FilterChip>
          {CONTEXTS.map((c) => (
            <FilterChip
              key={c.value}
              href={href({ context: context === c.value ? undefined : c.value })}
              active={context === c.value}
            >
              {c.label} context
            </FilterChip>
          ))}
          {params.provider && (
            <FilterChip href={href({ provider: undefined })} active>
              Provider: {params.provider} ✕
            </FilterChip>
          )}
        </nav>
      </div>

      {data.length === 0 ? (
        <p className="font-mono text-[13px] text-ink-3">No prices match these filters. Remove a filter to see more.</p>
      ) : (
        <DataTable
          caption={`${data.length} prices`}
          headers={["Model", "Provider", "Context", "Input", "Output", "Blended", "", "Checked"]}
          numeric={[2, 3, 4, 5]}
          highlight={sort === "blended" ? rowKey(0) : undefined}
          rows={data.map((row, i) => ({
            key: rowKey(i),
            cells: [
              <div key="m" className="flex items-center gap-2.5">
                <Logo name={row.maker.name} src={row.maker.logo_url} size="md" />
                <div className="min-w-0">
                  <Link
                    href={`/products/${row.product_slug}`}
                    className="font-sans text-[14px] text-ink hover:text-s-7 hover:underline"
                  >
                    {row.product_name}
                  </Link>
                  <div className="flex items-center gap-1.5 text-[12px] text-ink-3">
                    {row.maker.name}
                    {row.open_weights && <Badge tone="brand">open</Badge>}
                    {i === 0 && sort === "blended" && <Badge tone="solid">lowest</Badge>}
                  </div>
                </div>
              </div>,
              <ProviderLink
                key="p"
                slug={row.provider.slug}
                name={row.provider.name}
                logoUrl={row.provider.logo_url}
              />,
              <Price key="c">{formatTokens(row.context_window)}</Price>,
              <Price key="i">{formatPrice(row.price_input_per_mtok, row.currency)}</Price>,
              <Price key="o">{formatPrice(row.price_output_per_mtok, row.currency)}</Price>,
              <Price key="b" strong>
                {formatPrice(row.blended_per_mtok, row.currency)}
              </Price>,
              <span key="bar" className="hidden w-[126px] md:block">
                <PixelBar
                  value={row.blended_per_mtok ?? 0}
                  max={max}
                  cells={17}
                  tone={i === 0 && sort === "blended" ? "lead" : "muted"}
                  label={`${formatPrice(row.blended_per_mtok, row.currency)} blended per million tokens`}
                />
              </span>,
              <CheckedOn key="c2" date={row.observed_at} source={row.source_url} />,
            ],
          }))}
        />
      )}
    </div>
  );
}
