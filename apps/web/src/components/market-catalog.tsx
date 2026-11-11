import Link from "next/link";
import { DataTable } from "@/components/data-table";
import { RowLink, gpuHref, modelHref } from "@/components/market";
import { Price, SectionHeader, Stat } from "@/components/ui/primitives";
import type { components } from "@inference-hub/api-client";
import { formatCount, formatPrice, formatTokens } from "@/lib/format";

type Provider = components["schemas"]["MarketProvider"];
type ModelRow = components["schemas"]["ModelIndexRow"];
type GpuRow = components["schemas"]["GpuIndexRow"];

/**
 * Everything a provider sells, as its adapter reports it: counts that link to
 * the filtered market lists, and the first models with their market prices.
 */
export function MarketCatalog({
  name,
  provider,
  models,
  gpus,
}: {
  name: string;
  provider: Provider;
  models: { items: ModelRow[]; total: number };
  gpus: GpuRow[];
}) {
  const slug = provider.organization.slug;
  return (
    <section className="flex flex-col gap-4">
      <SectionHeader title={`Catalog at ${name}`} />
      <p className="max-w-[70ch] text-[14px] leading-6 text-ink-2">
        Everything {name} lists in its public catalog, collected by the hub&apos;s worker. Prices in
        the table are the whole market&apos;s, so you can see where {name} stands.
      </p>
      <div className="grid grid-cols-2 border border-line bg-wash sm:grid-cols-4">
        <Stat value={formatCount(provider.models)} label="chat models" href={`/market/models?provider=${slug}`} />
        <Stat value={formatCount(provider.media_models)} label="image, video, speech, embedding" />
        <Stat value={formatCount(provider.gpu_configurations)} label="GPU configurations" />
        <Stat value={formatCount(provider.cheapest_count)} label="cheapest in the market on" />
      </div>
      {models.items.length > 0 && (
        <DataTable
          caption={
            <span>
              {formatCount(models.total)} chat models, the most widely sold first ·{" "}
              <Link href={`/market/models?provider=${slug}`} className="text-s-7 underline">
                see all
              </Link>
            </span>
          }
          headers={["Model", "Providers", "Lowest blend", "Median blend", "Context"]}
          numeric={[1, 2, 3, 4]}
          rows={models.items.map((m) => ({
            key: m.model_key,
            cells: [
              <RowLink key="m" href={modelHref(m.model_key)}>
                {m.model_key}
              </RowLink>,
              <Price key="n">{m.providers}</Price>,
              <Price key="b" strong>
                {formatPrice(m.min_blended_per_mtok)}
              </Price>,
              <Price key="md">{formatPrice(m.median_blended_per_mtok)}</Price>,
              <Price key="c">{formatTokens(m.max_context_window)}</Price>,
            ],
          }))}
        />
      )}
      {gpus.length > 0 && (
        <DataTable
          caption={`${gpus.length} GPU types`}
          headers={["GPU", "Clouds", "Lowest", "Median"]}
          numeric={[1, 2, 3]}
          rows={gpus.map((g) => ({
            key: g.gpu_type,
            cells: [
              <RowLink key="g" href={gpuHref(g.gpu_type)}>
                {g.gpu_type}
              </RowLink>,
              <Price key="n">{g.providers}</Price>,
              <Price key="lo" strong>
                {formatPrice(g.min_per_gpu_hour)}
              </Price>,
              <Price key="md">{formatPrice(g.median_per_gpu_hour)}</Price>,
            ],
          }))}
        />
      )}
    </section>
  );
}
