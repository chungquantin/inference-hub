import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { FundingTable, PortfolioList } from "@/components/funding";
import { NewsList } from "@/components/news";
import { ProductList } from "@/components/product-list";
import { FactStrip, ProfileHeader } from "@/components/profile";
import { Badge, SectionHeader, buttonClass } from "@/components/ui/primitives";
import type { FundingRound } from "@inference-hub/api-client";
import { api } from "@/lib/api";
import { formatDate, formatMoney } from "@/lib/format";
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
    <article className="flex flex-col gap-8">
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
      <ProfileHeader
        name={org.name}
        logoUrl={org.logo_url}
        subtitle={org.tagline}
        badges={org.kinds.map((kind) => (
          <Badge key={kind}>{ORG_KIND_LABELS[kind]}</Badge>
        ))}
        actions={
          org.website ? (
            <a href={org.website} rel="noopener" target="_blank" className={buttonClass("secondary")}>
              Visit {new URL(org.website).hostname}
            </a>
          ) : undefined
        }
      />

      <FactStrip
        facts={[
          { label: "Founded", value: org.founded_year?.toString() },
          { label: "Headquarters", value: org.hq_country ?? undefined },
          { label: "Status", value: org.status === "active" ? undefined : org.status },
          { label: "Products", value: org.products.length || undefined },
          { label: "Serves", value: org.served_products.length || undefined },
          { label: "Raised", value: raised(org.funding) },
        ]}
      />

      {org.description && (
        <section className="max-w-[65ch] text-[15px] leading-relaxed text-ink-2">{org.description}</section>
      )}

      {org.products.length > 0 && (
        <section className="flex flex-col gap-3">
          <SectionHeader title="Products" />
          <ProductList products={org.products} showMaker={false} />
        </section>
      )}

      {org.served_products.length > 0 && (
        <section className="flex flex-col gap-3">
          <SectionHeader title={`Sold by ${org.name}`} />
          <ProductList products={org.served_products} />
        </section>
      )}

      {org.funding.length > 0 && (
        <section className="flex flex-col gap-3">
          <SectionHeader title="Funding" />
          <FundingTable rounds={org.funding} />
        </section>
      )}

      {org.portfolio.length > 0 && (
        <section className="flex flex-col gap-3">
          <SectionHeader title="Portfolio in the hub" />
          <PortfolioList companies={org.portfolio} />
        </section>
      )}

      {news.data && news.data.items.length > 0 && (
        <section className="flex flex-col gap-3">
          <SectionHeader
            title="News"
            href={news.data.total > news.data.items.length ? `/news?organization=${org.slug}` : undefined}
            linkLabel={`All ${news.data.total} stories`}
          />
          <NewsList items={news.data.items} />
        </section>
      )}

      <footer className="flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-line pt-4 text-sm text-ink-2">
        {org.categories.map((category) => (
          <Link key={category.id} href={`/categories/${category.slug}`} className="hover:text-accent">
            {category.name}
          </Link>
        ))}
        {links.map(([name, href]) => (
          <a key={name} href={href} rel="noopener" target="_blank" className="capitalize hover:text-accent">
            {name}
          </a>
        ))}
        <span className="ml-auto text-ink-3">Updated {formatDate(org.updated_at)}</span>
      </footer>
    </article>
  );
}

/** Money raised across rounds, when they are all in one currency. */
function raised(rounds: FundingRound[]): string | undefined {
  const known = rounds.filter((r) => r.amount != null);
  if (known.length === 0) return undefined;
  const currencies = new Set(known.map((r) => r.currency));
  if (currencies.size !== 1) return undefined;
  const total = known.reduce((sum, r) => sum + (r.amount ?? 0), 0);
  return formatMoney(total, known[0].currency);
}
