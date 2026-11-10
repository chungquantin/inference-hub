import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { FundingTable, PortfolioList } from "@/components/funding";
import { NewsList } from "@/components/news";
import { ProductList } from "@/components/product-list";
import { api } from "@/lib/api";
import { ORG_KIND_LABELS } from "@/lib/labels";
import { SITE_URL, jsonLd } from "@/lib/site";

export const revalidate = 300; // seconds; must be a literal for Next.js

type Props = { params: Promise<{ slug: string }> };

async function loadOrganization(slug: string) {
  const { data, response } = await api.GET("/v1/organizations/{slug}", {
    params: { path: { slug } },
  });
  if (response.status === 404) notFound();
  if (!data) throw new Error(`Could not load organization ${slug}`);
  return data;
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const org = await loadOrganization((await params).slug);
  return {
    title: org.name,
    description: org.tagline ?? undefined,
    alternates: { canonical: `/organizations/${org.slug}` },
  };
}

export default async function OrganizationPage({ params }: Props) {
  const { slug } = await params;
  const [org, news] = await Promise.all([
    loadOrganization(slug),
    api.GET("/v1/news", { params: { query: { organization: slug, limit: 8 } } }),
  ]);
  const links = Object.entries(org.links);

  return (
    <article className="flex flex-col gap-10">
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={jsonLd({
          "@type": "Organization",
          name: org.name,
          description: org.tagline ?? undefined,
          url: `${SITE_URL}/organizations/${org.slug}`,
          sameAs: [org.website, ...links.map(([, href]) => href)].filter(Boolean),
          foundingDate: org.founded_year ? String(org.founded_year) : undefined,
        })}
      />
      <header className="flex flex-col gap-3">
        <h1 className="text-3xl font-semibold tracking-tight">{org.name}</h1>
        {org.tagline && <p className="text-lg text-zinc-600 dark:text-zinc-400">{org.tagline}</p>}
        <ul className="flex flex-wrap gap-1.5">
          {org.kinds.map((kind) => (
            <li
              key={kind}
              className="rounded bg-zinc-100 px-2 py-0.5 text-xs text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300"
            >
              {ORG_KIND_LABELS[kind]}
            </li>
          ))}
        </ul>
      </header>

      <dl className="grid grid-cols-2 gap-4 text-sm sm:grid-cols-4">
        <Fact label="Founded" value={org.founded_year?.toString()} />
        <Fact label="Headquarters" value={org.hq_country ?? undefined} />
        <Fact label="Status" value={org.status} />
        <Fact
          label="Website"
          value={
            org.website ? (
              <a href={org.website} rel="noopener" target="_blank" className="underline">
                {new URL(org.website).hostname}
              </a>
            ) : undefined
          }
        />
      </dl>

      {org.description && (
        <section className="max-w-3xl leading-relaxed text-zinc-700 dark:text-zinc-300">
          {org.description}
        </section>
      )}

      <section className="flex flex-col gap-3">
        <h2 className="text-xl font-semibold">Products</h2>
        <ProductList products={org.products} showMaker={false} />
      </section>

      {org.served_products.length > 0 && (
        <section className="flex flex-col gap-3">
          <h2 className="text-xl font-semibold">Serves</h2>
          <p className="text-sm text-zinc-500">Products from other makers that {org.name} offers.</p>
          <ProductList products={org.served_products} />
        </section>
      )}

      {org.funding.length > 0 && (
        <section className="flex flex-col gap-3">
          <h2 className="text-xl font-semibold">Funding</h2>
          <FundingTable rounds={org.funding} />
        </section>
      )}

      {org.portfolio.length > 0 && (
        <section className="flex flex-col gap-1">
          <h2 className="text-xl font-semibold">Portfolio</h2>
          <p className="text-sm text-zinc-500">Companies in the hub that {org.name} invested in.</p>
          <PortfolioList companies={org.portfolio} />
        </section>
      )}

      {news.data && news.data.items.length > 0 && (
        <section className="flex flex-col gap-1">
          <h2 className="text-xl font-semibold">News</h2>
          <NewsList items={news.data.items} />
        </section>
      )}

      <footer className="flex flex-wrap gap-4 text-sm text-zinc-600 dark:text-zinc-400">
        {org.categories.map((category) => (
          <Link key={category.id} href={`/categories/${category.slug}`} className="hover:underline">
            {category.name}
          </Link>
        ))}
        {links.map(([name, href]) => (
          <a key={name} href={href} rel="noopener" target="_blank" className="hover:underline">
            {name}
          </a>
        ))}
        <span className="ml-auto text-zinc-500">
          Updated {new Date(org.updated_at).toLocaleDateString("en-US", { dateStyle: "medium" })}
        </span>
      </footer>
    </article>
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
