"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { MetricSummary } from "@inference-hub/api-client";
import { GROUP_ORDER, metricHref } from "@/lib/metrics";

type Item = { href: string; label: string };
type Group = { title: string; items: Item[] };

const FIXED: Group[] = [
  {
    title: "Prices",
    items: [
      { href: "/analytics", label: "Overview" },
      { href: "/analytics/models", label: "Models" },
      { href: "/analytics/gpus", label: "GPUs" },
      { href: "/analytics/media", label: "Image, video, speech" },
      { href: "/analytics/providers", label: "Providers" },
    ],
  },
  {
    title: "Analysis",
    items: [
      { href: "/analytics/rankings", label: "Rankings" },
      { href: "/analytics/curves", label: "Forward curves" },
    ],
  },
];

/**
 * The index of Analytics: every page of the section in one place, so there is a
 * single way in. A column on wide screens that stays in view; a row of chips that
 * scrolls sideways on narrow ones.
 */
export function AnalyticsRail({ metrics }: { metrics: MetricSummary[] }) {
  const path = usePathname();
  const signals: Group[] = [
    { title: "Signals", items: [{ href: "/analytics/metrics", label: "All signals" }] },
    ...GROUP_ORDER.map((g) => ({
      title: g,
      items: metrics.filter((m) => m.group === g).map((m) => ({ href: metricHref(m.key), label: m.label })),
    })).filter((g) => g.items.length),
  ];
  const on = (href: string) => (href === "/analytics" || href === "/analytics/metrics" ? path === href : path === href || path.startsWith(`${href}/`));
  const groups = [...FIXED, ...signals];
  return (
    <nav aria-label="Analytics" className="lg:sticky lg:top-20 lg:self-start">
      <div className="-mx-4 flex gap-1 overflow-x-auto px-4 pb-2 lg:mx-0 lg:flex-col lg:gap-5 lg:overflow-visible lg:px-0 lg:pb-0">
        {groups.map((g) => (
          <div key={g.title} className="flex gap-1 lg:flex-col lg:gap-0.5">
            <p className="hidden px-3 pt-1 pb-1 text-[13px] text-ink-3 lg:block">{g.title}</p>
            {g.items.map((i) => (
              <Link
                key={i.href}
                href={i.href}
                aria-current={on(i.href) ? "page" : undefined}
                className={`inline-flex h-9 shrink-0 items-center border px-3 text-[14px] whitespace-nowrap lg:border-0 ${
                  on(i.href) ? "border-s-7 bg-s-1 font-medium text-s-8" : "border-line text-ink-2 hover:bg-paper hover:text-ink"
                }`}
              >
                {i.label}
              </Link>
            ))}
          </div>
        ))}
      </div>
    </nav>
  );
}
