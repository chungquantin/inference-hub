import Link from "next/link";
import { CategoryGrid } from "@/components/category-grid";
import { SearchForm } from "@/components/listing";
import { OrganizationGrid } from "@/components/organization-card";
import { ProductList } from "@/components/product-list";
import { api } from "@/lib/api";

export const revalidate = 300; // seconds; must be a literal for Next.js

export default async function HomePage() {
  const [categories, orgs, models] = await Promise.all([
    api.GET("/v1/categories"),
    api.GET("/v1/organizations", { params: { query: { limit: 6 } } }),
    api.GET("/v1/products", { params: { query: { type: "model", limit: 6 } } }),
  ]);

  return (
    <div className="flex flex-col gap-14">
      <section className="flex flex-col gap-4">
        <h1 className="text-4xl font-semibold tracking-tight">The map of the inference market</h1>
        <p className="max-w-2xl text-lg text-zinc-600 dark:text-zinc-400">
          Model providers, LLM gateways, open-source engines, GPU clouds, hyperscalers and the
          investors behind them, in one place.
        </p>
        <div className="mt-2">
          <SearchForm action="/organizations" placeholder="Search companies and projects" />
        </div>
      </section>

      <section className="flex flex-col gap-4">
        <h2 className="text-xl font-semibold">Categories</h2>
        {categories.data ? (
          <CategoryGrid categories={categories.data} />
        ) : (
          <p className="text-sm text-red-600">Could not load categories.</p>
        )}
      </section>

      <section className="flex flex-col gap-4">
        <SectionHeader title="Latest models" href="/products?type=model" total={models.data?.total} />
        {models.data ? (
          <ProductList products={models.data.items} />
        ) : (
          <p className="text-sm text-red-600">Could not load models.</p>
        )}
      </section>

      <section className="flex flex-col gap-4">
        <SectionHeader title="In the directory" href="/organizations" total={orgs.data?.total} />
        {orgs.data ? (
          <OrganizationGrid orgs={orgs.data.items} />
        ) : (
          <p className="text-sm text-red-600">Could not load organizations.</p>
        )}
      </section>
    </div>
  );
}

function SectionHeader({ title, href, total }: { title: string; href: string; total?: number }) {
  return (
    <div className="flex items-baseline justify-between">
      <h2 className="text-xl font-semibold">{title}</h2>
      <Link href={href} className="text-sm text-zinc-600 hover:underline dark:text-zinc-400">
        View all{total != null ? ` ${total}` : ""} →
      </Link>
    </div>
  );
}
