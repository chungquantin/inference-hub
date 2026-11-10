import Link from "next/link";
import type { Category } from "@inference-hub/api-client";

export function CategoryGrid({ categories }: { categories: Category[] }) {
  return (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      {categories.map((category) => (
        <Link
          key={category.id}
          href={`/categories/${category.slug}`}
          className="group rounded-lg border border-zinc-200 p-4 transition-colors hover:border-zinc-400 dark:border-zinc-800 dark:hover:border-zinc-600"
        >
          <div className="flex items-baseline justify-between">
            <h3 className="font-medium group-hover:underline">{category.name}</h3>
            <span className="font-mono text-xs text-zinc-500">{category.organization_count}</span>
          </div>
          {category.description && (
            <p className="mt-1 text-sm text-zinc-600 dark:text-zinc-400">{category.description}</p>
          )}
        </Link>
      ))}
    </div>
  );
}
