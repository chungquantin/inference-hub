import Link from "next/link";
import type { Offering } from "@inference-hub/api-client";
import { formatDate, formatPrice } from "@/lib/format";

/**
 * Where a product can be bought. Token-priced offerings (model APIs) and
 * time-priced ones (GPU instances) get separate tables, since their columns differ.
 */
export function OfferingsTable({ offerings }: { offerings: Offering[] }) {
  if (offerings.length === 0) {
    return <p className="text-sm text-zinc-500">No pricing recorded yet.</p>;
  }
  const tokenPriced = offerings.filter((o) => o.price_per_hour == null);
  const timePriced = offerings.filter((o) => o.price_per_hour != null);

  return (
    <div className="flex flex-col gap-6">
      {tokenPriced.length > 0 && (
        <Table
          caption="API pricing per million tokens"
          headers={["Provider", "Input", "Output", "Region", "Checked"]}
          numeric={[1, 2]}
          rows={tokenPriced.map((o) => [
            <ProviderCell key="p" offering={o} />,
            formatPrice(o.price_input_per_mtok, o.currency),
            formatPrice(o.price_output_per_mtok, o.currency),
            o.region ?? "—",
            <Checked key="c" offering={o} />,
          ])}
        />
      )}
      {timePriced.length > 0 && (
        <Table
          caption="Instance pricing"
          headers={["Provider", "Configuration", "Per hour", "Per GPU-hour", "Region", "Checked"]}
          numeric={[2, 3]}
          rows={timePriced.map((o) => [
            <ProviderCell key="p" offering={o} />,
            [o.gpu_count && `${o.gpu_count}×`, o.gpu_type].filter(Boolean).join(" ") || "—",
            formatPrice(o.price_per_hour, o.currency),
            formatPrice(
              o.price_per_hour != null && o.gpu_count ? o.price_per_hour / o.gpu_count : null,
              o.currency,
            ),
            o.region ?? "—",
            <Checked key="c" offering={o} />,
          ])}
        />
      )}
    </div>
  );
}

function ProviderCell({ offering }: { offering: Offering }) {
  return (
    <Link href={`/organizations/${offering.provider.slug}`} className="font-medium hover:underline">
      {offering.provider.name}
    </Link>
  );
}

function Checked({ offering }: { offering: Offering }) {
  const date = formatDate(offering.observed_at);
  return offering.source_url ? (
    <a href={offering.source_url} rel="noopener" target="_blank" className="underline">
      {date}
    </a>
  ) : (
    <>{date}</>
  );
}

function Table({
  caption,
  headers,
  rows,
  numeric,
}: {
  caption: string;
  headers: string[];
  rows: React.ReactNode[][];
  numeric: number[];
}) {
  const align = (i: number) => (numeric.includes(i) ? "text-right tabular-nums" : "text-left");
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <caption className="mb-2 text-left text-xs text-zinc-500">{caption}</caption>
        <thead>
          <tr className="border-b border-zinc-200 text-zinc-500 dark:border-zinc-800">
            {headers.map((h, i) => (
              <th key={h} scope="col" className={`py-2 pr-4 font-normal ${align(i)}`}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-zinc-100 dark:divide-zinc-900">
          {rows.map((cells, r) => (
            <tr key={r}>
              {cells.map((cell, i) => (
                <td key={i} className={`py-2 pr-4 ${align(i)}`}>
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
