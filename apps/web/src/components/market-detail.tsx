import { DataTable } from "@/components/data-table";
import { CheckedOn, ProviderLink } from "@/components/price-cells";
import { PixelBar } from "@/components/ui/pixel";
import { Badge, Price } from "@/components/ui/primitives";
import type { components } from "@inference-hub/api-client";
import { Capabilities } from "@/components/capabilities";
import { formatUnitPrice } from "@/components/market";
import { formatPrice, formatTokens } from "@/lib/format";

type Listing = components["schemas"]["MarketListing"];

/** Every provider's price for one GPU type or model, cheapest first. */
export function MarketListings({ listings, kind }: { listings: Listing[]; kind: "gpu" | "model" | "media" }) {
  const unit = (l: Listing) =>
    (kind === "gpu" ? l.price_per_gpu_hour : kind === "media" ? l.price_per_unit : l.blended_per_mtok) ?? 0;
  const max = Math.max(...listings.map(unit), 0);
  const key = (l: Listing, i: number) => `${l.adapter}:${l.external_id}:${l.variant ?? ""}:${l.gpu_count ?? ""}:${i}`;
  const headers =
    kind === "gpu"
      ? ["Provider", "Configuration", "GPUs", "Per GPU-hour", "", "Per hour", "Checked"]
      : kind === "media"
        ? ["Provider", "Listed as", "Price", "", "Checked"]
        : ["Provider", "Listed as", "Capabilities", "Input", "Output", "Blend", "", "Context", "Checked"];
  return (
    <DataTable
      caption={`${listings.length} prices, cheapest first`}
      headers={headers}
      numeric={kind === "gpu" ? [2, 3, 5] : kind === "media" ? [2] : [3, 4, 5, 7]}
      highlight={listings.length > 1 ? key(listings[0], 0) : undefined}
      rows={listings.map((l, i) => {
        const provider = (
          <span key="p" className="flex items-center gap-2 whitespace-nowrap">
            <ProviderLink slug={l.provider.slug} name={l.provider.name} logoUrl={l.provider.logo_url} />
            {i === 0 && listings.length > 1 && <Badge tone="solid">lowest</Badge>}
          </span>
        );
        const bar = (
          <span key="bar" className="hidden w-[120px] md:block">
            <PixelBar value={unit(l)} max={max} cells={16} tone={i === 0 ? "lead" : "muted"} label={formatPrice(unit(l))} />
          </span>
        );
        const via = (
          <span key="v" className="text-ink-2">
            {l.name}
            {l.variant && <span className="text-ink-3"> · {l.variant}</span>}
          </span>
        );
        const checked = <CheckedOn key="c" date={l.last_seen_at} source={l.source_url} />;
        return {
          key: key(l, i),
          cells:
            kind === "media"
              ? [
                  provider,
                  via,
                  <Price key="u" strong>
                    {formatUnitPrice(l.price_per_unit, l.unit)}
                  </Price>,
                  bar,
                  checked,
                ]
              : kind === "gpu"
              ? [
                  provider,
                  via,
                  <Price key="n">{l.gpu_count}</Price>,
                  <Price key="g" strong>
                    {formatPrice(l.price_per_gpu_hour)}
                  </Price>,
                  bar,
                  <Price key="h">{formatPrice(l.price_per_hour)}</Price>,
                  checked,
                ]
              : [
                  provider,
                  via,
                  <Capabilities key="cap" caps={l.capabilities} />,
                  <Price key="i">{formatPrice(l.price_input_per_mtok)}</Price>,
                  <Price key="o">{formatPrice(l.price_output_per_mtok)}</Price>,
                  <Price key="b" strong>
                    {formatPrice(l.blended_per_mtok)}
                  </Price>,
                  bar,
                  <Price key="x">{formatTokens(l.context_window)}</Price>,
                  checked,
                ],
        };
      })}
    />
  );
}
