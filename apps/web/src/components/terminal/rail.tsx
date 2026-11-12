"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { MetricSummary } from "@inference-hub/api-client";
import { GROUP_ORDER, metricHref } from "@/lib/metrics";

/**
 * The Terminal's index: metrics grouped by what they measure. A column on wide
 * screens that stays in view; a row of chips that scrolls sideways on narrow ones.
 */
export function Rail({ metrics }: { metrics: MetricSummary[] }) {
  const path = usePathname();
  const groups = GROUP_ORDER.map((g) => ({ g, items: metrics.filter((m) => m.group === g) })).filter((x) => x.items.length);
  return (
    <nav aria-label="Metrics" className="lg:sticky lg:top-20 lg:self-start">
      <Link
        href="/market/terminal"
        aria-current={path === "/market/terminal" ? "page" : undefined}
        className={`mb-1 hidden h-9 items-center px-3 text-[14px] font-medium lg:flex ${
          path === "/market/terminal" ? "bg-s-1 text-s-8" : "text-ink-2 hover:bg-paper hover:text-ink"
        }`}
      >
        Overview
      </Link>
      <div className="-mx-4 flex gap-1 overflow-x-auto px-4 pb-2 lg:mx-0 lg:flex-col lg:gap-5 lg:overflow-visible lg:px-0 lg:pb-0">
        <Link
          href="/market/terminal"
          className={`inline-flex h-9 shrink-0 items-center border px-3 text-[14px] lg:hidden ${
            path === "/market/terminal" ? "border-s-7 bg-s-1 text-s-8" : "border-line text-ink-2"
          }`}
        >
          Overview
        </Link>
        {groups.map(({ g, items }) => (
          <div key={g} className="flex gap-1 lg:flex-col lg:gap-0.5">
            <p className="hidden px-3 pt-1 pb-1 text-[13px] text-ink-3 lg:block">{g}</p>
            {items.map((m) => {
              const on = path === metricHref(m.key);
              return (
                <Link
                  key={m.key}
                  href={metricHref(m.key)}
                  aria-current={on ? "page" : undefined}
                  className={`inline-flex h-9 shrink-0 items-center gap-2 border px-3 text-[14px] whitespace-nowrap lg:border-0 ${
                    on ? "border-s-7 bg-s-1 font-medium text-s-8" : "border-line text-ink-2 hover:bg-paper hover:text-ink"
                  }`}
                >
                  {m.label}
                </Link>
              );
            })}
          </div>
        ))}
      </div>
    </nav>
  );
}
