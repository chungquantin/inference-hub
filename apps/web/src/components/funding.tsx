import Link from "next/link";
import type { FundingRound, PortfolioCompany } from "@inference-hub/api-client";
import { DataTable } from "@/components/data-table";
import { Logo } from "@/components/ui/logo";
import { Badge, Price } from "@/components/ui/primitives";
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
            <a key="r" href={r.source_url} rel="noopener" target="_blank" className="text-ink underline hover:text-s-7">
              {formatRound(r.round)}
            </a>
          ) : (
            <span key="r">{formatRound(r.round)}</span>
          ),
          <span key="d" className="text-ink-2">
            {formatDate(r.announced_on)}
          </span>,
          <Price key="a" strong>
            {formatMoney(r.amount, r.currency)}
          </Price>,
          <Price key="v">{r.valuation != null ? formatMoney(r.valuation, r.currency) : "—"}</Price>,
          <div key="i" className="flex flex-col gap-1.5 py-2">
            <div className="flex flex-wrap gap-1">
              {r.investors.map((inv) => (
                <Link
                  key={inv.slug}
                  href={`/organizations/${inv.slug}`}
                  className={`inline-flex h-6 items-center gap-1.5 pr-2 text-[12px] ${inv.lead ? "bg-s-1 text-s-8 hover:bg-s-2" : "bg-field text-ink-2 hover:bg-s-2"}`}
                >
                  <Logo name={inv.name} src={inv.logo_url} size="xs" />
                  {inv.name}
                  {inv.lead && <span className="text-s-7">lead</span>}
                </Link>
              ))}
            </div>
            {r.notes && <span className="font-sans text-[12px] text-ink-3">{r.notes}</span>}
          </div>,
        ],
      }))}
    />
  );
}

/** Companies an investor backed, with the rounds it joined. */
export function PortfolioList({ companies }: { companies: PortfolioCompany[] }) {
  return (
    <ul className="border border-line bg-wash">
      {companies.map((c) => (
        <li key={c.slug} className="border-b border-line last:border-0">
          <Link href={`/organizations/${c.slug}`} className="group flex flex-wrap items-center gap-4 px-4 py-3 hover:bg-s-1">
            <Logo name={c.name} src={c.logo_url} size="md" />
            <span className="flex-1 text-[15px] text-ink group-hover:text-s-7">{c.name}</span>
            <span className="flex flex-wrap justify-end gap-1">
              {c.rounds.map((r) => (
                <Badge key={`${r.round}:${r.announced_on}`} tone={r.lead ? "brand" : "neutral"}>
                  {formatRound(r.round)} {formatMoney(r.amount, r.currency)}
                  {r.lead ? " lead" : ""}
                </Badge>
              ))}
            </span>
          </Link>
        </li>
      ))}
    </ul>
  );
}
