import type { Offering } from "@inference-hub/api-client";
import { DataTable } from "@/components/data-table";
import { CheckedOn, ProviderLink } from "@/components/price-cells";
import { Price } from "@/components/ui/primitives";
import { formatPrice } from "@/lib/format";

/**
 * Where a product can be bought. Token-priced offerings (model APIs) and
 * time-priced ones (GPU instances) get separate tables, since their columns differ.
 */
export function OfferingsTable({ offerings }: { offerings: Offering[] }) {
  if (offerings.length === 0) {
    return <p className="font-mono text-[13px] text-ink-3">No provider sells this yet, as far as the hub knows.</p>;
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
              <ProviderLink key="p" slug={o.provider.slug} name={o.provider.name} logoUrl={o.provider.logo_url} />,
              <Price key="i">{formatPrice(o.price_input_per_mtok, o.currency)}</Price>,
              <Price key="o" strong>
                {formatPrice(o.price_output_per_mtok, o.currency)}
              </Price>,
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
              <ProviderLink key="p" slug={o.provider.slug} name={o.provider.name} logoUrl={o.provider.logo_url} />,
              [o.gpu_count && `${o.gpu_count}×`, o.gpu_type].filter(Boolean).join(" ") || "—",
              <Price key="h">{formatPrice(o.price_per_hour, o.currency)}</Price>,
              <Price key="g" strong>
                {formatPrice(
                  o.price_per_hour != null && o.gpu_count ? o.price_per_hour / o.gpu_count : null,
                  o.currency,
                )}
              </Price>,
              o.region ?? "—",
              <CheckedOn key="c" date={o.observed_at} source={o.source_url} />,
            ],
          }))}
        />
      )}
    </div>
  );
}
