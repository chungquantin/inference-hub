import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { api } from "@/lib/api";
import { ORG_KIND_LABELS } from "@/lib/labels";

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
  return { title: org.name, description: org.tagline ?? undefined };
}

export default async function OrganizationPage({ params }: Props) {
  const org = await loadOrganization((await params).slug);
  const links = Object.entries(org.links as Record<string, string>);

  return (
    <article className="flex flex-col gap-10">
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
        {org.products.length === 0 ? (
          <p className="text-sm text-zinc-500">No products listed yet.</p>
        ) : (
          <ul className="divide-y divide-zinc-200 dark:divide-zinc-800">
            {org.products.map((product) => (
              <li key={product.id} className="flex items-baseline justify-between gap-4 py-3">
                <div>
                  <p className="font-medium">{product.name}</p>
                  {product.tagline && (
                    <p className="text-sm text-zinc-600 dark:text-zinc-400">{product.tagline}</p>
                  )}
                </div>
                <span className="font-mono text-xs text-zinc-500">{product.product_type}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

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
