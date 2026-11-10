"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const LINKS = [
  { href: "/compare", label: "Compare" },
  { href: "/organizations", label: "Directory" },
  { href: "/products", label: "Products" },
  { href: "/news", label: "News" },
  { href: "/categories", label: "Categories" },
];

/** Primary navigation; marks the section of the current page. */
export function MainNav() {
  const pathname = usePathname();
  return (
    <nav aria-label="Main" className="flex items-center gap-1 overflow-x-auto text-sm">
      {LINKS.map(({ href, label }) => {
        const active = pathname === href || pathname.startsWith(`${href}/`);
        return (
          <Link
            key={href}
            href={href}
            aria-current={active ? "page" : undefined}
            className={
              active
                ? "rounded-[6px] bg-inset px-3 py-1.5 font-medium text-ink"
                : "rounded-[6px] px-3 py-1.5 text-ink-2 hover:bg-hover hover:text-ink"
            }
          >
            {label}
          </Link>
        );
      })}
    </nav>
  );
}
