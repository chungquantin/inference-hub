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
            <a
              key="r"
              href={r.source_url}
              rel="noopener"
              target="_blank"
              className="font-medium text-ink underline decoration-line-strong underline-offset-2 hover:text-accent"
            >
              {formatRound(r.round)}
            </a>
          ) : (
            <span key="r" className="font-medium">
              {formatRound(r.round)}
            </span>
          ),
          <span key="d" className="text-ink-2">
            {formatDate(r.announced_on)}
          </span>,
          <Price key="a" strong>
            {formatMoney(r.amount, r.currency)}
          </Price>,
          <Price key="v">{r.valuation != null ? formatMoney(r.valuation, r.currency) : "—"}</Price>,
          <div key="i" className="flex flex-col gap-1.5">
            <div className="flex flex-wrap gap-1.5">
              {r.investors.map((inv) => (
                <Link
                  key={inv.slug}
                  href={`/organizations/${inv.slug}`}
                  className="inline-flex items-center gap-1.5 rounded-full bg-inset py-0.5 pr-2 pl-0.5 text-xs text-ink-2 hover:text-ink"
                >
                  <Logo name={inv.name} src={inv.logo_url} size="sm" />
                  {inv.name}
                  {inv.lead && <span className="font-medium text-accent-ink">lead</span>}
                </Link>
              ))}
            </div>
            {r.notes && <span className="text-xs text-ink-3">{r.notes}</span>}
          </div>,
        ],
      }))}
    />
  );
}

/** Companies an investor backed, with the rounds it joined. */
export function PortfolioList({ companies }: { companies: PortfolioCompany[] }) {
  return (
    <ul className="overflow-hidden rounded-[10px] bg-surface shadow-elev-1">
      {companies.map((c) => (
        <li key={c.slug} className="border-b border-line last:border-0">
          <Link href={`/organizations/${c.slug}`} className="group flex items-center gap-3 px-4 py-3 hover:bg-hover/50">
            <Logo name={c.name} src={c.logo_url} size="md" />
            <span className="flex-1 font-medium text-ink group-hover:text-accent">{c.name}</span>
            <span className="flex flex-wrap justify-end gap-1.5">
              {c.rounds.map((r) => (
                <Badge key={`${r.round}:${r.announced_on}`} tone={r.lead ? "accent" : "neutral"}>
                  {formatRound(r.round)} · {formatMoney(r.amount, r.currency)}
                  {r.lead ? " · lead" : ""}
                </Badge>
              ))}
            </span>
          </Link>
        </li>
      ))}
    </ul>
  );
}
