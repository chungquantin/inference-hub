"use client";

import Link from "next/link";
import { useState } from "react";
import { HyperLink, type HyperLinkProps } from "@/components/hyper-link";

export type CheapRow = {
  /** The GPU or model, as a typed link entity (logo and page follow from it). */
  entity: HyperLinkProps;
  /** Key of the row. */
  name: string;
  lo: string;
  median: string;
  /** The cheapest seller. */
  at: { slug: string; name: string; logo: string | null };
};
export type CheapTab = { id: string; label: string; unit: string; total: string; allHref: string; allLabel: string; rows: CheapRow[] };

/** "Cheapest right now": the lowest prices for the most widely sold GPUs and models, in tabs. */
export function CheapestTable({ tabs }: { tabs: CheapTab[] }) {
  const [id, setId] = useState(tabs[0]?.id);
  const tab = tabs.find((t) => t.id === id) ?? tabs[0];
  if (!tab) return null;
  return (
    <div className="border border-line-strong bg-wash">
      <div className="flex h-[52px] items-center justify-between border-b border-line pr-4 pl-1.5">
        <div role="tablist" aria-label="What to compare" className="flex gap-0.5">
          {tabs.map((t) => (
            <button
              key={t.id}
              role="tab"
              aria-selected={t.id === tab.id}
              onClick={() => setId(t.id)}
              className={`h-9 px-3.5 text-[15px] font-medium ${t.id === tab.id ? "bg-paper text-ink" : "text-ink-2 hover:text-ink"}`}
            >
              {t.label}
            </button>
          ))}
        </div>
        <span className="text-[13px] text-ink-3">{tab.unit}</span>
      </div>
      <table className="w-full border-collapse text-[15px]">
        <thead>
          <tr className="text-left text-[13px] text-ink-3">
            <th className="px-4 py-2.5 font-medium">{tab.id === "gpus" ? "GPU" : "Model"}</th>
            <th className="px-4 py-2.5 text-right font-medium">Lowest</th>
            <th className="hidden px-4 py-2.5 text-right font-medium sm:table-cell">Median</th>
            <th className="hidden px-4 py-2.5 font-medium md:table-cell">Cheapest at</th>
          </tr>
        </thead>
        <tbody>
          {tab.rows.map((r) => (
            <tr key={r.name} className="border-t border-line hover:bg-ground">
              <td className="h-[52px] px-4">
                <HyperLink {...r.entity} className="inline-flex items-center gap-2.5 font-medium hover:text-s-7 hover:underline" />
              </td>
              <td className="px-4 text-right font-mono font-medium text-lo tabular">{r.lo}</td>
              <td className="hidden px-4 text-right font-mono tabular sm:table-cell">{r.median}</td>
              <td className="hidden px-4 md:table-cell">
                <HyperLink type="org" slug={r.at.slug} name={r.at.name} logoUrl={r.at.logo} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="flex items-center justify-between border-t border-line bg-ground px-4 py-3.5 text-[14px] text-ink-3">
        <span>{tab.total}</span>
        <Link href={tab.allHref} className="text-ink underline underline-offset-4 hover:text-s-7">
          {tab.allLabel}
        </Link>
      </div>
    </div>
  );
}
