import Link from "next/link";
import type { Category } from "@inference-hub/api-client";

export function CategoryGrid({ categories }: { categories: Category[] }) {
  return (
    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
      {categories.map((category) => (
        <Link
          key={category.id}
          href={`/categories/${category.slug}`}
          className="group flex flex-col gap-1 rounded-[10px] bg-surface p-4 shadow-elev-1 transition-shadow hover:shadow-elev-2"
        >
          <div className="flex items-baseline justify-between gap-2">
            <h3 className="font-display font-semibold tracking-[-0.01em] text-ink group-hover:text-accent">
              {category.name}
            </h3>
            <span className="font-mono text-sm tabular-nums text-ink-3">{category.organization_count}</span>
          </div>
          {category.description && <p className="text-sm text-ink-2">{category.description}</p>}
        </Link>
      ))}
    </div>
  );
}
