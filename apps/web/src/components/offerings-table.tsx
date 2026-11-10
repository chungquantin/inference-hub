import type { Offering } from "@inference-hub/api-client";
import { DataTable } from "@/components/data-table";
import { CheckedOn, ProviderLink } from "@/components/price-cells";
import { formatPrice } from "@/lib/format";

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
        <DataTable
          caption="API pricing per million tokens"
          headers={["Provider", "Input", "Output", "Region", "Checked"]}
          numeric={[1, 2]}
          rows={tokenPriced.map((o) => ({
            key: o.id,
            cells: [
              <ProviderLink key="p" slug={o.provider.slug} name={o.provider.name} />,
              formatPrice(o.price_input_per_mtok, o.currency),
              formatPrice(o.price_output_per_mtok, o.currency),
              o.region ?? "—",
              <CheckedOn key="c" date={o.observed_at} source={o.source_url} />,
            ],
          }))}
        />
      )}
      {timePriced.length > 0 && (
        <DataTable
          caption="Instance pricing"
          headers={["Provider", "Configuration", "Per hour", "Per GPU-hour", "Region", "Checked"]}
          numeric={[2, 3]}
          rows={timePriced.map((o) => ({
            key: o.id,
            cells: [
              <ProviderLink key="p" slug={o.provider.slug} name={o.provider.name} />,
              [o.gpu_count && `${o.gpu_count}×`, o.gpu_type].filter(Boolean).join(" ") || "—",
              formatPrice(o.price_per_hour, o.currency),
              formatPrice(
                o.price_per_hour != null && o.gpu_count ? o.price_per_hour / o.gpu_count : null,
                o.currency,
              ),
              o.region ?? "—",
              <CheckedOn key="c" date={o.observed_at} source={o.source_url} />,
            ],
          }))}
        />
      )}
    </div>
  );
}
