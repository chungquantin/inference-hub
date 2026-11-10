import Link from "next/link";
import { CategoryGrid } from "@/components/category-grid";
import { OrganizationGrid } from "@/components/organization-card";
import { api } from "@/lib/api";

export const revalidate = 300; // seconds; must be a literal for Next.js

export default async function HomePage() {
  const [categories, orgs] = await Promise.all([
    api.GET("/v1/categories"),
    api.GET("/v1/organizations", { params: { query: { limit: 6 } } }),
  ]);

  return (
    <div className="flex flex-col gap-14">
      <section className="flex flex-col gap-4">
        <h1 className="text-4xl font-semibold tracking-tight">The map of the inference market</h1>
        <p className="max-w-2xl text-lg text-zinc-600 dark:text-zinc-400">
          Model providers, LLM gateways, open-source engines, GPU clouds, hyperscalers and the
          investors behind them, in one place.
        </p>
        <form action="/organizations" className="mt-2 flex max-w-xl gap-2">
          <input
            type="search"
            name="q"
            placeholder="Search companies and projects"
            className="flex-1 rounded-md border border-zinc-300 bg-transparent px-3 py-2 text-sm outline-none focus:border-zinc-500 dark:border-zinc-700"
          />
          <button
            type="submit"
            className="rounded-md bg-zinc-900 px-4 py-2 text-sm font-medium text-white dark:bg-zinc-100 dark:text-zinc-900"
          >
            Search
          </button>
        </form>
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
        <div className="flex items-baseline justify-between">
          <h2 className="text-xl font-semibold">In the directory</h2>
          <Link href="/organizations" className="text-sm text-zinc-600 hover:underline dark:text-zinc-400">
            View all{orgs.data ? ` ${orgs.data.total}` : ""} →
          </Link>
        </div>
        {orgs.data ? (
          <OrganizationGrid orgs={orgs.data.items} />
        ) : (
          <p className="text-sm text-red-600">Could not load organizations.</p>
        )}
      </section>
    </div>
  );
}
