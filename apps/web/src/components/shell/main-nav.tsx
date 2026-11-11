"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { NAV_SECTIONS, sectionOf } from "@/lib/nav";
import { openCommandPalette } from "@/components/shell/command-palette";

/**
 * Primary navigation: one tab per section, each opening a menu of its pages
 * on hover or click. Below `md` the tabs collapse into a full-width menu.
 */
export function MainNav() {
  const pathname = usePathname();
  const active = sectionOf(pathname);
  const [open, setOpen] = useState<string | null>(null);
  const [mobile, setMobile] = useState(false);
  const navRef = useRef<HTMLElement>(null);

  // Close menus on navigation.
  const [lastPath, setLastPath] = useState(pathname);
  if (pathname !== lastPath) {
    setLastPath(pathname);
    setOpen(null);
    setMobile(false);
  }

  // Close on outside click and on Escape.
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (navRef.current && !navRef.current.contains(e.target as Node)) setOpen(null);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(null);
        setMobile(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, []);

  return (
    <>
      <nav ref={navRef} aria-label="Main" className="hidden h-full flex-1 items-stretch md:flex">
        {NAV_SECTIONS.map((section) => {
          const isActive = active === section.href;
          const single = section.items.length === 1;
          const isOpen = open === section.href;
          return (
            <div
              key={section.href}
              className="relative flex items-stretch"
              onMouseEnter={() => !single && setOpen(section.href)}
              onMouseLeave={() => setOpen((o) => (o === section.href ? null : o))}
            >
              {single ? (
                <Link
                  href={section.href}
                  aria-current={isActive ? "page" : undefined}
                  className={tabClass(isActive)}
                >
                  {section.label}
                </Link>
              ) : (
                <button
                  type="button"
                  aria-expanded={isOpen}
                  aria-haspopup="true"
                  aria-controls={`menu-${section.label}`}
                  onClick={() => setOpen(isOpen ? null : section.href)}
                  className={tabClass(isActive)}
                >
                  {section.label}
                  <Caret open={isOpen} />
                </button>
              )}
              {!single && isOpen && (
                <div
                  id={`menu-${section.label}`}
                  className="absolute top-full left-0 z-50 w-[340px] border border-line bg-wash p-1.5 shadow-[0_12px_32px_-12px_rgba(14,19,48,0.25)]"
                >
                  <ul className="flex flex-col">
                    {section.items.map((item) => {
                      const current = pathname === item.href;
                      return (
                        <li key={item.href}>
                          <Link
                            href={item.href}
                            aria-current={current ? "page" : undefined}
                            className={`flex flex-col gap-0.5 px-3 py-2.5 hover:bg-s-1 focus-visible:bg-s-1 ${
                              current ? "bg-s-1" : ""
                            }`}
                          >
                            <span className="text-[15px] font-medium text-ink">{item.label}</span>
                            <span className="text-[13px] leading-4 text-ink-3">{item.description}</span>
                          </Link>
                        </li>
                      );
                    })}
                  </ul>
                </div>
              )}
            </div>
          );
        })}
      </nav>

      <div className="ml-auto flex items-center gap-2">
        <button
          type="button"
          onClick={openCommandPalette}
          className="flex h-10 items-center gap-3 border border-line bg-wash px-3 text-left text-[14px] text-ink-3 hover:border-[color:var(--s-7)] hover:text-ink lg:w-[300px]"
          aria-label="Search the hub"
        >
          <SearchIcon />
          <span className="hidden flex-1 truncate lg:inline">Search models, GPUs, companies</span>
          <kbd className="hidden border border-line bg-wash px-1.5 font-mono text-[11px] text-ink-3 lg:inline">⌘K</kbd>
        </button>
        <button
          type="button"
          className="flex h-10 w-10 items-center justify-center border border-line md:hidden"
          aria-expanded={mobile}
          aria-controls="mobile-menu"
          aria-label={mobile ? "Close menu" : "Open menu"}
          onClick={() => setMobile((m) => !m)}
        >
          <MenuIcon open={mobile} />
        </button>
      </div>

      {mobile && (
        <div
          id="mobile-menu"
          className="fixed inset-x-0 top-16 bottom-0 z-40 overflow-y-auto border-t border-line bg-wash md:hidden"
        >
          <nav aria-label="Main" className="flex flex-col gap-6 px-6 py-6">
            {NAV_SECTIONS.map((section) => (
              <div key={section.href} className="flex flex-col gap-1">
                <span className="text-[13px] font-semibold text-s-7">{section.label}</span>
                {section.items.map((item) => (
                  <Link
                    key={item.href}
                    href={item.href}
                    aria-current={pathname === item.href ? "page" : undefined}
                    className="flex flex-col py-2 aria-[current=page]:text-s-7"
                  >
                    <span className="text-[16px]">{item.label}</span>
                    <span className="text-[13px] text-ink-3">{item.description}</span>
                  </Link>
                ))}
              </div>
            ))}
          </nav>
        </div>
      )}
    </>
  );
}

function tabClass(active: boolean): string {
  return `flex items-center gap-1.5 px-3.5 text-[15px] whitespace-nowrap ${
    active
      ? "font-semibold text-ink shadow-[inset_0_-2px_0_var(--s-7)]"
      : "font-medium text-ink-2 hover:text-ink"
  }`;
}

function Caret({ open }: { open: boolean }) {
  return (
    <svg width="8" height="8" viewBox="0 0 8 8" aria-hidden className={open ? "rotate-180" : ""}>
      <path d="M0 2h8L4 6z" fill="currentColor" />
    </svg>
  );
}

export function SearchIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden shapeRendering="crispEdges">
      <path d="M6 1a5 5 0 1 0 3.2 8.8L12.6 13l.9-.9-3.3-3.4A5 5 0 0 0 6 1zm0 1.5a3.5 3.5 0 1 1 0 7 3.5 3.5 0 0 1 0-7z" fill="currentColor" />
    </svg>
  );
}

function MenuIcon({ open }: { open: boolean }) {
  return open ? (
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden>
      <path d="M2 2l10 10M12 2L2 12" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  ) : (
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden shapeRendering="crispEdges">
      <path d="M1 3h12M1 7h12M1 11h12" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}
