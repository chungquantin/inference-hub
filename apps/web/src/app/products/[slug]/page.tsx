import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import type { ModelSpec, RepoStats } from "@inference-hub/api-client";
import { OfferingsTable } from "@/components/offerings-table";
import { FactStrip, ProfileHeader } from "@/components/profile";
import { Badge, SectionHeader, buttonClass } from "@/components/ui/primitives";
import { PriceHistoryChart } from "@/components/price-history-chart";
import { api } from "@/lib/api";
import { formatCount, formatDate, formatParams, formatTokens } from "@/lib/format";
import { PRODUCT_TYPE_LABELS } from "@/lib/labels";
import { SITE_URL, jsonLd } from "@/lib/site";

export const revalidate = 300; // seconds; must be a literal for Next.js

type Props = { params: Promise<{ slug: string }> };

async function loadProduct(slug: string) {
  const { data, response } = await api.GET("/v1/products/{slug}", {
    params: { path: { slug } },
  });
  if (response.status === 404) notFound();
  if (!data) throw new Error(`Could not load product ${slug}`);
  return data;
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const product = await loadProduct((await params).slug);
  return {
    title: `${product.name} by ${product.organization.name}`,
    description: product.tagline ?? undefined,
    alternates: { canonical: `/products/${product.slug}` },
  };
}

export default async function ProductPage({ params }: Props) {
  const { slug } = await params;
  const [product, history] = await Promise.all([
    loadProduct(slug),
    api.GET("/v1/products/{slug}/price-history", { params: { path: { slug } } }),
  ]);
  // Rendered on the server per request (and per revalidation), so "today" is the render time.
  const renderedAt = new Date(history.response.headers.get("date") ?? 0).getTime() || 0;
  const historyUnit =
    product.offerings.some((o) => o.price_per_hour != null) ? "Price per hour" : "Output price per million tokens";
  const metadata = Object.entries(product.metadata).filter(
    ([, v]) => typeof v === "string" || typeof v === "number" || typeof v === "boolean",
  );

  return (
    <article className="flex flex-col gap-8">
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={jsonLd({
          "@type": "Product",
          name: product.name,
          description: product.tagline ?? undefined,
          url: `${SITE_URL}/products/${product.slug}`,
          brand: { "@type": "Organization", name: product.organization.name },
        })}
      />

      <nav className="-mb-4 text-sm text-ink-3" aria-label="Breadcrumb">
        <Link href="/products" className="hover:text-ink">
          Products
        </Link>
        <span className="px-1.5">/</span>
        <Link href={`/products?type=${product.product_type}`} className="hover:text-ink">
          {PRODUCT_TYPE_LABELS[product.product_type]}
        </Link>
      </nav>

      <ProfileHeader
        name={product.name}
        logoName={product.organization.name}
        logoUrl={product.organization.logo_url}
        subtitle={
          <>
            by{" "}
            <Link
              href={`/organizations/${product.organization.slug}`}
              className="font-medium text-ink hover:text-s-7"
            >
              {product.organization.name}
            </Link>
            {product.tagline && <span className="text-ink-2">. {product.tagline}</span>}
          </>
        }
        badges={
          <>
            <Badge>{PRODUCT_TYPE_LABELS[product.product_type]}</Badge>
            {product.model?.open_weights && <Badge tone="brand">Open weights</Badge>}
            {product.repo?.archived && <Badge tone="warn">Archived</Badge>}
          </>
        }
        actions={
          product.url ? (
            <a href={product.url} rel="noopener" target="_blank" className={buttonClass("secondary")}>
              Visit {new URL(product.url).hostname}
            </a>
          ) : undefined
        }
      />

      <FactStrip
        facts={[
          { label: "Released", value: product.launched_at ? formatDate(product.launched_at) : undefined },
          ...(product.model ? modelFacts(product.model) : []),
          ...metadata.map(([key, value]) => ({ label: humanize(key), value: String(value) })),
        ]}
      />

      {product.description && (
        <section className="max-w-[65ch] text-[15px] leading-relaxed text-ink-2">{product.description}</section>
      )}

      <section className="flex flex-col gap-3">
        <SectionHeader title="Where to get it" />
        <OfferingsTable offerings={product.offerings} />
      </section>

      {history.data && history.data.length > 1 && (
        <section className="flex flex-col gap-3">
          <SectionHeader title="Price history" />
          <PriceHistoryChart points={history.data} unit={historyUnit} now={renderedAt} />
        </section>
      )}

      {product.repo && <RepoSection repo={product.repo} />}
    </article>
  );
}

function modelFacts(model: ModelSpec): { label: string; value: React.ReactNode }[] {
  const params =
    model.active_params_b != null
      ? `${formatParams(model.parameters_b)} (${formatParams(model.active_params_b)} active)`
      : model.parameters_b != null
        ? formatParams(model.parameters_b)
        : undefined;
  return [
    { label: "Parameters", value: params },
    {
      label: "Context window",
      value: model.context_window ? `${formatTokens(model.context_window)} tokens` : undefined,
    },
    { label: "Input", value: model.modalities.length ? model.modalities.join(", ") : undefined },
    { label: "License", value: model.license ?? undefined },
  ];
}

function RepoSection({ repo }: { repo: RepoStats }) {
  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-baseline justify-between gap-4">
        <h2 className="font-display text-lg font-semibold tracking-[-0.01em] text-ink">On GitHub</h2>
        <a
          href={`https://github.com/${repo.repo}`}
          rel="noopener"
          target="_blank"
          className="font-mono text-sm text-ink-2 hover:text-s-7"
        >
          {repo.repo}
        </a>
      </div>
      <FactStrip
        facts={[
          {
            label: "Stars",
            value: (
              <>
                {formatCount(repo.stars)}
                {repo.stars_30d != null && (
                  <span className="ml-1.5 text-sm font-normal text-s-7">
                    +{formatCount(repo.stars_30d)} in 30 days
                  </span>
                )}
              </>
            ),
          },
          { label: "Forks", value: formatCount(repo.forks) },
          { label: "License", value: repo.license ?? undefined },
          {
            label: "Latest release",
            value: repo.latest_release
              ? `${repo.latest_release}${repo.released_at ? `, ${formatDate(repo.released_at)}` : ""}`
              : undefined,
          },
        ]}
      />
      <p className="text-xs text-ink-3">
        {repo.archived && "Archived and no longer maintained. "}Synced {formatDate(repo.synced_at)}.
      </p>
    </section>
  );
}

function humanize(key: string): string {
  const words = key.replace(/_/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}
