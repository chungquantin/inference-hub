import Link from "next/link";
import type { FundingRound, PortfolioCompany } from "@inference-hub/api-client";
import { DataTable } from "@/components/data-table";
import { formatDate, formatMoney, formatRound } from "@/lib/format";

/** Rounds an organization raised, newest first. */
export function FundingTable({ rounds }: { rounds: FundingRound[] }) {
  return (
    <DataTable
      headers={["Round", "Announced", "Amount", "Valuation", "Investors"]}
      numeric={[2, 3]}
      rows={rounds.map((r) => ({
        key: `${r.round}:${r.announced_on}`,
        cells: [
          r.source_url ? (
            <a key="r" href={r.source_url} rel="noopener" target="_blank" className="font-medium underline">
              {formatRound(r.round)}
            </a>
          ) : (
            <span key="r" className="font-medium">
              {formatRound(r.round)}
            </span>
          ),
          formatDate(r.announced_on),
          formatMoney(r.amount, r.currency),
          r.valuation != null ? `${formatMoney(r.valuation, r.currency)} post` : "—",
          <div key="i" className="flex flex-col gap-0.5">
            {r.investors.length > 0 && (
              <span>
                {r.investors.map((inv, i) => (
                  <span key={inv.slug}>
                    {i > 0 && ", "}
                    <Link href={`/organizations/${inv.slug}`} className="hover:underline">
                      {inv.name}
                    </Link>
                    {inv.lead && <span className="text-xs text-zinc-500"> (lead)</span>}
                  </span>
                ))}
              </span>
            )}
            {r.notes && <span className="text-xs text-zinc-500">{r.notes}</span>}
          </div>,
        ],
      }))}
    />
  );
}

/** Companies an investor backed, with the rounds it joined. */
export function PortfolioList({ companies }: { companies: PortfolioCompany[] }) {
  return (
    <ul className="divide-y divide-zinc-200 dark:divide-zinc-800">
      {companies.map((c) => (
        <li key={c.slug} className="flex flex-wrap items-baseline justify-between gap-2 py-3">
          <Link href={`/organizations/${c.slug}`} className="font-medium hover:underline">
            {c.name}
          </Link>
          <span className="text-sm text-zinc-600 dark:text-zinc-400">
            {c.rounds
              .map(
                (r) =>
                  `${formatRound(r.round)}${r.lead ? " (lead)" : ""} · ${formatMoney(r.amount, r.currency)} · ${formatDate(r.announced_on)}`,
              )
              .join("; ")}
          </span>
        </li>
      ))}
    </ul>
  );
}
