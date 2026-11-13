/**
 * The site map. The header shows the sections; each opens a menu of its
 * pages. The footer and the command palette read the same list.
 */
export type NavItem = { href: string; label: string; description: string };
export type NavSection = { href: string; label: string; items: NavItem[] };

export const NAV_SECTIONS: NavSection[] = [
  {
    href: "/directory",
    label: "Directory",
    items: [{ href: "/directory", label: "Search the directory", description: "GPUs, models and companies in one searchable table" }],
  },
  {
    href: "/analytics",
    label: "Analytics",
    items: [{ href: "/analytics", label: "Analytics", description: "Prices, rankings, forward curves and signals for models, GPUs and providers" }],
  },
  {
    href: "/compare",
    label: "Compare",
    items: [
      { href: "/compare/models", label: "Tracked model APIs", description: "Curated prices with history" },
      { href: "/compare/gpus", label: "Tracked GPU clouds", description: "Curated per-GPU prices" },
      { href: "/compare/open-source", label: "Open source", description: "Engines by stars and releases" },
    ],
  },
  {
    href: "/organizations",
    label: "Explore",
    items: [
      { href: "/landscape", label: "Landscape", description: "Every company on one map, with funding where sourced" },
      { href: "/organizations", label: "Organizations", description: "Labs, clouds, gateways, investors" },
      { href: "/products", label: "Products", description: "Models, APIs, GPUs, projects" },
      { href: "/categories", label: "Categories", description: "The market's taxonomy" },
    ],
  },
  {
    href: "/news",
    label: "News",
    items: [{ href: "/news", label: "Latest news", description: "Tagged with the companies they cover" }],
  },
];

/** Flat list of every page, for the footer and the command palette. */
export const NAV_LINKS: NavItem[] = NAV_SECTIONS.flatMap((s) => s.items);

/** The section a path belongs to, for the active state. */
export function sectionOf(pathname: string): string | undefined {
  if (pathname.startsWith("/products") || pathname.startsWith("/categories")) return "/organizations";
  return NAV_SECTIONS.find((s) => pathname === s.href || pathname.startsWith(`${s.href}/`))?.href;
}
