import type { Metadata } from "next";
import { CategoryGrid } from "@/components/category-grid";
import { api } from "@/lib/api";

export const revalidate = 300; // seconds; must be a literal for Next.js
export const metadata: Metadata = { title: "Categories" };

export default async function CategoriesPage() {
  const { data, error } = await api.GET("/v1/categories");
  if (error || !data) throw new Error("Could not load categories");

  return (
    <div className="flex flex-col gap-6">
      <h1 className="text-3xl font-semibold tracking-tight">Categories</h1>
      <CategoryGrid categories={data} />
    </div>
  );
}
