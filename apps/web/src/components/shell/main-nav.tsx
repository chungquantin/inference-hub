"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { NAV_LINKS } from "@/lib/nav";


/** Primary navigation in mono; the current section is underlined in the brand color. */
export function MainNav() {
  const pathname = usePathname();
  return (
    <nav aria-label="Main" className="flex h-full items-stretch overflow-x-auto">
      {NAV_LINKS.map(({ href, label }) => {
        const active = pathname === href || pathname.startsWith(`${href}/`);
        return (
          <Link
            key={href}
            href={href}
            aria-current={active ? "page" : undefined}
            className={`flex items-center border-b-2 px-3 font-mono text-[13px] whitespace-nowrap ${
              active ? "border-s-7 text-ink" : "border-transparent text-ink-2 hover:text-ink"
            }`}
          >
            {label}
          </Link>
        );
      })}
    </nav>
  );
}
