import Link from "next/link";
import type { Category } from "@inference-hub/api-client";

/** Top-level categories on a shared 1px grid, each with its sub-categories listed below. */
export function CategoryGrid({ categories }: { categories: Category[] }) {
  const children = new Map<string, Category[]>();
  for (const category of categories) {
    if (category.parent_id) children.set(category.parent_id, [...(children.get(category.parent_id) ?? []), category]);
  }
  const roots = categories.filter((c) => !c.parent_id);
  return (
    <div className="grid grid-cols-1 pt-px pl-px sm:grid-cols-2 lg:grid-cols-4">
      {roots.map((category) => (
        <div key={category.id} className="group -mt-px -ml-px flex flex-col border border-line bg-wash hover:z-10 hover:border-s-7">
          <Link href={`/categories/${category.slug}`} className="flex flex-1 flex-col gap-2 p-5">
            <span className="font-mono text-[28px] leading-8 text-s-7 tabular">
              {String(category.organization_count).padStart(2, "0")}
            </span>
            <h3 className="font-display text-[18px] leading-6 tracking-[-0.01em] text-ink group-hover:text-s-7">
              {category.name}
            </h3>
            {category.description && <p className="text-[14px] leading-5 text-ink-2">{category.description}</p>}
          </Link>
          {(children.get(category.id) ?? []).length > 0 && (
            <ul className="flex flex-wrap gap-x-3 gap-y-1 border-t border-line px-5 py-3 font-mono text-[12px] leading-4">
              {(children.get(category.id) ?? []).map((sub) => (
                <li key={sub.id}>
                  <Link href={`/categories/${sub.slug}`} className="text-ink-2 hover:text-s-7">
                    {sub.name} <span className="text-ink-3 tabular">{sub.organization_count}</span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>
      ))}
    </div>
  );
}
