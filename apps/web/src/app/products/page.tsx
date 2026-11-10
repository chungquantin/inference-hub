import type { Metadata } from "next";
import type { ProductType } from "@inference-hub/api-client";
import { FilterChip, Pagination, SearchForm, listingHref, parsePage } from "@/components/listing";
import { ProductList } from "@/components/product-list";
import { PageHeader } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { PRODUCT_TYPE_LABELS, isProductType } from "@/lib/labels";

export const metadata: Metadata = {
  title: "Products",
  description: "Models, inference APIs, GPU instances and open-source projects.",
  alternates: { canonical: "/products" },
};

const PAGE_SIZE = 30;

type Props = {
  searchParams: Promise<{ q?: string; type?: string; open?: string; page?: string }>;
};

export default async function ProductsPage({ searchParams }: Props) {
  const { q, type, open, page } = await searchParams;
  const activeType = isProductType(type) ? type : undefined;
  const openWeights = open === "1" ? true : undefined;
  const pageNumber = parsePage(page);

  const { data, error } = await api.GET("/v1/products", {
    params: {
      query: {
        q: q || undefined,
        type: activeType,
        open_weights: openWeights,
        limit: PAGE_SIZE,
        offset: (pageNumber - 1) * PAGE_SIZE,
      },
    },
  });
  if (error || !data) throw new Error("Could not load products");

  const current = { q, type: activeType, open: openWeights ? "1" : undefined };
  const href = (overrides: Record<string, string | undefined>) =>
    listingHref("/products", current, overrides);

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-col gap-4">
        <PageHeader title="Products" lead="Models, inference APIs, GPU instances, hardware and open-source projects." />
        <SearchForm
          defaultValue={q}
          placeholder="Search models, APIs and projects"
          hidden={current}
        />
        <nav className="flex flex-wrap gap-2 text-sm" aria-label="Filter by type">
          <FilterChip href={href({ type: undefined, open: undefined })} active={!activeType}>
            All
          </FilterChip>
          {(Object.keys(PRODUCT_TYPE_LABELS) as ProductType[]).map((t) => (
            <FilterChip
              key={t}
              href={href({ type: t, open: t === "model" ? current.open : undefined })}
              active={t === activeType}
            >
              {PRODUCT_TYPE_LABELS[t]}
            </FilterChip>
          ))}
        </nav>
        {activeType === "model" && (
          <nav className="flex gap-2 text-sm" aria-label="Filter by weights">
            <FilterChip
              href={href({ open: openWeights ? undefined : "1" })}
              active={Boolean(openWeights)}
            >
              Open weights only
            </FilterChip>
          </nav>
        )}
      </header>

      <p className="text-sm text-ink-3">
        {data.total} {data.total === 1 ? "result" : "results"}
      </p>
      <ProductList products={data.items} empty="No products found." />
      <Pagination
        page={pageNumber}
        lastPage={Math.max(1, Math.ceil(data.total / PAGE_SIZE))}
        hrefFor={(p) => href({ page: String(p) })}
      />
    </div>
  );
}
