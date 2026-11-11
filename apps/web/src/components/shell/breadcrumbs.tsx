import Link from "next/link";

/** Where a page sits in the site: `Market / Models / deepseek-v3.2`. */
export function Breadcrumbs({ items }: { items: { href?: string; label: string }[] }) {
  return (
    <nav aria-label="Breadcrumb" className="font-mono text-[12px] text-ink-3">
      <ol className="flex flex-wrap items-center gap-1.5">
        {items.map((item, i) => (
          <li key={i} className="flex items-center gap-1.5">
            {i > 0 && <span aria-hidden>/</span>}
            {item.href && i < items.length - 1 ? (
              <Link href={item.href} className="hover:text-s-7 hover:underline">
                {item.label}
              </Link>
            ) : (
              <span aria-current={i === items.length - 1 ? "page" : undefined} className="text-ink-2">
                {item.label}
              </span>
            )}
          </li>
        ))}
      </ol>
    </nav>
  );
}
