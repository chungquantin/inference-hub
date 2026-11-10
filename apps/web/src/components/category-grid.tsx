import Link from "next/link";
import type { Category } from "@inference-hub/api-client";

/** Categories on a shared 1px grid, with the organization count in mono. */
export function CategoryGrid({ categories }: { categories: Category[] }) {
  return (
    <div className="grid grid-cols-1 pt-px pl-px sm:grid-cols-2 lg:grid-cols-4">
      {categories.map((category) => (
        <Link
          key={category.id}
          href={`/categories/${category.slug}`}
          className="group -mt-px -ml-px flex flex-col gap-2 border border-line bg-wash p-5 hover:z-10 hover:border-s-7"
        >
          <span className="font-mono text-[28px] leading-8 text-s-7 tabular">
            {String(category.organization_count).padStart(2, "0")}
          </span>
          <h3 className="font-display text-[18px] leading-6 tracking-[-0.01em] text-ink group-hover:text-s-7">
            {category.name}
          </h3>
          {category.description && <p className="text-[14px] leading-5 text-ink-2">{category.description}</p>}
        </Link>
      ))}
    </div>
  );
}
