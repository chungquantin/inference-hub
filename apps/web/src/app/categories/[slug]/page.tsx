import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { OrganizationGrid } from "@/components/organization-card";
import { api } from "@/lib/api";

export const revalidate = 300; // seconds; must be a literal for Next.js

type Props = { params: Promise<{ slug: string }> };

async function loadCategory(slug: string) {
  const { data, response } = await api.GET("/v1/categories/{slug}", {
    params: { path: { slug } },
  });
  if (response.status === 404) notFound();
  if (!data) throw new Error(`Could not load category ${slug}`);
  return data;
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const category = await loadCategory((await params).slug);
  return { title: category.name, description: category.description ?? undefined };
}

export default async function CategoryPage({ params }: Props) {
  const { slug } = await params;
  const [category, orgs] = await Promise.all([
    loadCategory(slug),
    api.GET("/v1/organizations", { params: { query: { category: slug, limit: 100 } } }),
  ]);
  if (!orgs.data) throw new Error(`Could not load organizations for ${slug}`);

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-col gap-2">
        <h1 className="text-3xl font-semibold tracking-tight">{category.name}</h1>
        {category.description && (
          <p className="text-zinc-600 dark:text-zinc-400">{category.description}</p>
        )}
      </header>
      <OrganizationGrid orgs={orgs.data.items} />
    </div>
  );
}
