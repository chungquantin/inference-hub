import type { MetadataRoute } from "next";
import { api } from "@/lib/api";
import { SITE_URL } from "@/lib/site";

export const revalidate = 3600; // seconds; must be a literal for Next.js

export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  const { data, error } = await api.GET("/v1/sitemap");
  if (error || !data) throw new Error("Could not load sitemap data");

  const staticPages: MetadataRoute.Sitemap = ["", "/categories", "/organizations", "/products"].map(
    (path) => ({ url: `${SITE_URL}${path}`, changeFrequency: "daily", priority: path ? 0.8 : 1 }),
  );
  const pages = (prefix: string, entries: typeof data.categories, priority: number) =>
    entries.map((e) => ({
      url: `${SITE_URL}/${prefix}/${e.slug}`,
      lastModified: e.updated_at,
      priority,
    }));

  return [
    ...staticPages,
    ...pages("categories", data.categories, 0.7),
    ...pages("organizations", data.organizations, 0.6),
    ...pages("products", data.products, 0.6),
  ];
}
