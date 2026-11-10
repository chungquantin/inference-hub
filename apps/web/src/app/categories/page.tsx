import type { Metadata } from "next";
import { CategoryGrid } from "@/components/category-grid";
import { PageHeader } from "@/components/ui/primitives";
import { api } from "@/lib/api";

export const revalidate = 300; // seconds; must be a literal for Next.js
export const metadata: Metadata = {
  title: "Categories",
  alternates: { canonical: "/categories" },
};

export default async function CategoriesPage() {
  const { data, error } = await api.GET("/v1/categories");
  if (error || !data) throw new Error("Could not load categories");

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Categories" />
      <CategoryGrid categories={data} />
    </div>
  );
}
