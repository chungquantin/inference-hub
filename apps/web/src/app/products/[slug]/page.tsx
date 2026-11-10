import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import type { ModelSpec } from "@inference-hub/api-client";
import { OfferingsTable } from "@/components/offerings-table";
import { api } from "@/lib/api";
import { formatDate, formatParams, formatTokens } from "@/lib/format";
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
  const product = await loadProduct((await params).slug);
  const metadata = Object.entries(product.metadata).filter(
    ([, v]) => typeof v === "string" || typeof v === "number" || typeof v === "boolean",
  );

  return (
    <article className="flex flex-col gap-10">
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

      <header className="flex flex-col gap-3">
        <nav className="text-sm text-zinc-500" aria-label="Breadcrumb">
          <Link href="/products" className="hover:underline">
            Products
          </Link>
          {" / "}
          <Link href={`/products?type=${product.product_type}`} className="hover:underline">
            {PRODUCT_TYPE_LABELS[product.product_type]}
          </Link>
        </nav>
        <h1 className="text-3xl font-semibold tracking-tight">{product.name}</h1>
        <p className="text-zinc-600 dark:text-zinc-400">
          by{" "}
          <Link
            href={`/organizations/${product.organization.slug}`}
            className="font-medium text-zinc-900 underline dark:text-zinc-100"
          >
            {product.organization.name}
          </Link>
          {product.tagline && <> · {product.tagline}</>}
        </p>
      </header>

      <dl className="grid grid-cols-2 gap-4 text-sm sm:grid-cols-4">
        <Fact label="Type" value={PRODUCT_TYPE_LABELS[product.product_type]} />
        <Fact label="Released" value={product.launched_at ? formatDate(product.launched_at) : undefined} />
        {product.model && <ModelFacts model={product.model} />}
        {metadata.map(([key, value]) => (
          <Fact key={key} label={humanize(key)} value={String(value)} />
        ))}
        {product.url && (
          <Fact
            label="Link"
            value={
              <a href={product.url} rel="noopener" target="_blank" className="underline">
                {new URL(product.url).hostname}
              </a>
            }
          />
        )}
      </dl>

      {product.description && (
        <section className="max-w-3xl leading-relaxed text-zinc-700 dark:text-zinc-300">
          {product.description}
        </section>
      )}

      <section className="flex flex-col gap-3">
        <h2 className="text-xl font-semibold">Where to get it</h2>
        <OfferingsTable offerings={product.offerings} />
      </section>
    </article>
  );
}

function ModelFacts({ model }: { model: ModelSpec }) {
  const params =
    model.active_params_b != null
      ? `${formatParams(model.parameters_b)} (${formatParams(model.active_params_b)} active)`
      : model.parameters_b != null
        ? formatParams(model.parameters_b)
        : undefined;
  return (
    <>
      <Fact label="Parameters" value={params} />
      <Fact
        label="Context window"
        value={model.context_window ? `${formatTokens(model.context_window)} tokens` : undefined}
      />
      <Fact label="Input" value={model.modalities.length ? model.modalities.join(", ") : undefined} />
      <Fact label="Weights" value={model.open_weights ? "Open" : "Closed"} />
      <Fact label="License" value={model.license ?? undefined} />
      {model.family && <Fact label="Family" value={model.family} />}
    </>
  );
}

function Fact({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div>
      <dt className="text-zinc-500">{label}</dt>
      <dd className="mt-0.5 font-medium">{value ?? "—"}</dd>
    </div>
  );
}

function humanize(key: string): string {
  const words = key.replace(/_/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}
