import type { Metadata } from "next";
import Link from "next/link";
import { DataTable } from "@/components/data-table";
import { RowLink, SpreadBar, ago, modelHref, spread } from "@/components/market";
import { HyperLink } from "@/components/hyper-link";
import { LiveDot, WindowFrame } from "@/components/ui/pixel";
import { Badge, PageHeader, Price, SectionHeader, Stat } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatCount, formatPrice, formatTokens } from "@/lib/format";

export const revalidate = 300; // seconds; must be a literal for Next.js

export const metadata: Metadata = {
  title: "Inference market",
  description:
    "Live prices of model APIs and GPU clouds across every provider we track: the cheapest offer, the median and the spread.",
  alternates: { canonical: "/analytics" },
};

export default async function MarketPage() {
  const { data, error } = await api.GET("/v1/market");
  if (error || !data) throw new Error("Could not load the market");
  const now = data.updated_at;
  const healthy = data.sources.filter((s) => s.healthy).length;

  return (
    <div className="flex flex-col gap-10">
      <PageHeader
        label="Analytics"
        title="The inference market"
        lead="Every price our adapters collect from model APIs, routers and GPU clouds, refreshed hourly to daily. Unit prices only: per GPU-hour, and per million tokens at a 3:1 input-to-output blend."
      >
        <span className="flex items-center gap-2 font-mono text-[13px] text-ink-3">
          <LiveDot /> updated {ago(now, now)}
        </span>
      </PageHeader>

      <section aria-label="Market totals" className="grid grid-cols-2 border border-line bg-wash sm:grid-cols-5">
        <Stat value={data.providers} label="providers tracked" href="/analytics/providers" />
        <Stat
          value={formatCount(data.distinct_models)}
          label={`chat models · ${formatCount(data.multi_provider_models)} on 2+ providers`}
          href="/analytics/models"
        />
        <Stat value={data.gpu_types} label={`GPU types · ${data.gpu_listings} configs`} href="/analytics/gpus" />
        <Stat
          value={formatCount(data.media_models)}
          label={`image, video, speech, embedding · ${formatCount(data.media_listings)} prices`}
          href="/analytics/media"
        />
        <Stat value={`${healthy}/${data.sources.length}`} label="sources healthy" href="#sources" />
      </section>

      <WindowFrame title="GPU index · per GPU-hour" meta={`${data.gpu_providers} clouds`}>
        <DataTable
          headers={["GPU", "Clouds", "Lowest", "Median", "Highest", "Spread", "", "Cheapest at"]}
          numeric={[1, 2, 3, 4, 5]}
          rows={data.gpu_index.map((row) => ({
            key: row.gpu_type,
            cells: [
              <HyperLink
                key="g"
                type="gpu"
                gpuType={row.gpu_type}
                className="inline-flex items-center gap-2 text-ink underline decoration-line-strong hover:text-s-7"
              />,
              <Price key="n">{row.providers}</Price>,
              <Price key="lo" strong>
                {formatPrice(row.min_per_gpu_hour)}
              </Price>,
              <Price key="md">{formatPrice(row.median_per_gpu_hour)}</Price>,
              <Price key="hi">{formatPrice(row.max_per_gpu_hour)}</Price>,
              <Price key="sp">{spread(row.min_per_gpu_hour, row.max_per_gpu_hour)}</Price>,
              <span key="bar" className="hidden w-[110px] md:block">
                <SpreadBar
                  min={row.min_per_gpu_hour}
                  median={row.median_per_gpu_hour}
                  max={row.max_per_gpu_hour}
                  unit="per GPU-hour"
                />
              </span>,
              <HyperLink
                key="c"
                type="org"
                slug={row.cheapest.slug}
                name={row.cheapest.name}
                logoUrl={row.cheapest.logo_url} />,
            ],
          }))}
        />
        <Link href="/analytics/gpus" className="block border-t border-line px-3 py-2 font-mono text-[13px] text-s-7 underline">
          All {data.gpu_types} GPU types
        </Link>
      </WindowFrame>

      <WindowFrame title="Model index · per million tokens" meta={`${data.model_providers} providers`}>
        <DataTable
          headers={["Model", "Providers", "Lowest in", "Lowest out", "Lowest blend", "Median blend", "Context", "Cheapest at"]}
          numeric={[1, 2, 3, 4, 5, 6]}
          rows={data.model_index.map((row) => ({
            key: row.model_key,
            cells: [
              <RowLink key="m" href={modelHref(row.model_key)}>
                {row.model_key}
              </RowLink>,
              <Price key="n">{row.providers}</Price>,
              <Price key="i">{formatPrice(row.min_input_per_mtok)}</Price>,
              <Price key="o">{formatPrice(row.min_output_per_mtok)}</Price>,
              <Price key="b" strong>
                {formatPrice(row.min_blended_per_mtok)}
              </Price>,
              <Price key="mb">{formatPrice(row.median_blended_per_mtok)}</Price>,
              <Price key="c">{formatTokens(row.max_context_window)}</Price>,
              <HyperLink
                key="p"
                type="org"
                slug={row.cheapest.slug}
                name={row.cheapest.name}
                logoUrl={row.cheapest.logo_url} />,
            ],
          }))}
        />
        <Link href="/analytics/models" className="block border-t border-line px-3 py-2 font-mono text-[13px] text-s-7 underline">
          All {formatCount(data.distinct_models)} chat models ({formatCount(data.multi_provider_models)} sold by 2+ providers)
        </Link>
      </WindowFrame>

      <section id="sources" className="flex flex-col gap-4">
        <SectionHeader title="Sources" count={data.sources.length} />
        <p className="max-w-[70ch] text-[14px] leading-6 text-ink-2">
          Each source is a provider&apos;s public price API or page. A source that fails keeps its
          last prices until it recovers; a run that returns far fewer prices than the last one is
          refused rather than stored.
        </p>
        <DataTable
          headers={["Provider", "Kind", "Prices", "Last success", "Status"]}
          numeric={[2]}
          rows={data.sources.map((s) => ({
            key: s.adapter,
            cells: [
              <HyperLink
                key="p"
                type="org"
                slug={s.organization.slug}
                name={s.organization.name}
                logoUrl={s.organization.logo_url} />,
              <span key="k" className="text-ink-3">
                {s.kind === "gpu" ? "GPU cloud" : s.kind === "model" ? "Model API" : "—"}
              </span>,
              <Price key="n">{formatCount(s.listings)}</Price>,
              <span key="t" className="text-ink-3">
                {ago(s.last_success_at, now)}
              </span>,
              s.healthy ? (
                <Badge key="s" tone="up">
                  ok
                </Badge>
              ) : (
                <Badge key="s" tone="warn">
                  failing
                </Badge>
              ),
            ],
          }))}
        />
      </section>
    </div>
  );
}
