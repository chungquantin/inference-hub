import Link from "next/link";
import type { ModelPrice, components } from "@inference-hub/api-client";

type MarketListing = components["schemas"]["MarketListing"];
import { CategoryGrid } from "@/components/category-grid";
import { NewsList } from "@/components/news";
import { Logo } from "@/components/ui/logo";
import { LiveDot, PixelBar, WindowFrame } from "@/components/ui/pixel";
import { HeroSearch } from "@/components/shell/hero-search";
import { SectionHeader, Stat, buttonClass } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { creatorName, modelLogoUrl } from "@/lib/catalog";
import { formatCount, formatDate, formatPrice } from "@/lib/format";

export const revalidate = 300; // seconds; must be a literal for Next.js

/** GPU types on the board, in order. */
const BOARD_GPUS = ["H100 SXM", "H200 SXM", "B200"];

export default async function HomePage() {
  const [models, categories, news, orgs, oss, market, cheapest] = await Promise.all([
    api.GET("/v1/compare/models", { params: { query: { sort: "blended" } } }),
    api.GET("/v1/categories"),
    api.GET("/v1/news", { params: { query: { limit: 6 } } }),
    api.GET("/v1/organizations", { params: { query: { limit: 1 } } }),
    api.GET("/v1/compare/open-source"),
    api.GET("/v1/market"),
    // The most widely sold models, cheapest first: what most people compare.
    api.GET("/v1/market/models", { params: { query: { min_providers: 8, sort: "price", limit: 8 } } }),
  ]);
  // Live prices of the board's GPUs, from every cloud the worker reads.
  const boards = await Promise.all(
    BOARD_GPUS.map((gpu) => api.GET("/v1/market/gpus/{gpu_type}", { params: { path: { gpu_type: gpu } } })),
  );
  const m = market.data;

  const modelPrices = models.data ?? [];
  // The cheapest price of each model, cheapest models first.
  const cheapestModels = Object.values(
    modelPrices.reduce<Record<string, ModelPrice>>((acc, m) => {
      if (m.blended_per_mtok == null || m.blended_per_mtok <= 0) return acc;
      const seen = acc[m.product_slug];
      if (!seen || (seen.blended_per_mtok ?? Infinity) > m.blended_per_mtok) acc[m.product_slug] = m;
      return acc;
    }, {}),
  )
    .sort((a, b) => (a.blended_per_mtok ?? 0) - (b.blended_per_mtok ?? 0))
    .slice(0, 6);
  const latest = m?.updated_at;

  return (
    <div className="flex flex-col gap-20">
      <section className="grid gap-10 lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]">
        <div className="flex min-w-0 flex-col gap-6">
          <p className="flex items-center gap-2 font-mono text-[13px] text-s-7">
            <LiveDot />
            {latest ? `Prices updated ${formatDate(latest)} · ${m?.providers} providers` : "Market prices"}
          </p>
          <h1 className="font-display text-[40px] leading-[44px] tracking-[-0.03em] text-ink sm:text-[56px] sm:leading-[60px]">
            What inference costs, and who sells it.
          </h1>
          <p className="max-w-[46ch] text-[17px] leading-7 text-ink-2">
            Every model API and GPU price we can read from the providers themselves, compared
            across the market, next to the companies, projects and investors behind them.
          </p>
          <HeroSearch />
          <div className="flex flex-wrap gap-3">
            <Link href="/market" className={buttonClass("primary")}>
              Open the market
            </Link>
            <Link href="/market/models?min=2&sort=price" className={buttonClass("secondary")}>
              Cheapest models
            </Link>
          </div>
        </div>

        <WindowFrame
          title="GPU price board / per GPU-hour"
          meta={<Link href="/market/gpus" className="hover:text-wash hover:underline">all GPUs</Link>}
          className="min-w-0"
        >
          {BOARD_GPUS.map((gpu, i) => (
            <GpuBoard key={gpu} gpu={gpu} listings={boards[i].data?.listings ?? []} />
          ))}
        </WindowFrame>
      </section>

      <section className="grid grid-cols-2 pt-px pl-px lg:grid-cols-5 [&>*]:-mt-px [&>*]:-ml-px [&>*]:border [&>*]:border-line">
        <Stat value={m ? formatCount(m.distinct_models) : "—"} label="Chat models priced" href="/market/models" />
        <Stat value={m?.gpu_types ?? "—"} label="GPU types" href="/market/gpus" />
        <Stat value={m ? formatCount(m.media_models) : "—"} label="Image, video, speech" href="/market/media" />
        <Stat value={m?.providers ?? "—"} label="Providers with live prices" href="/market/providers" />
        <Stat value={orgs.data?.total ?? "—"} label="Organizations" href="/organizations" />
      </section>

      {cheapest.data && cheapest.data.items.length > 0 && (
        <section className="flex flex-col gap-4">
          <SectionHeader
            title="Popular models, cheapest first"
            href="/market/models?min=2&sort=price"
            linkLabel="All comparable models"
          />
          <ul className="grid gap-px border border-line bg-line sm:grid-cols-2 lg:grid-cols-4">
            {cheapest.data.items.map((row) => (
              <li key={row.model_key} className="bg-wash">
                <Link href={`/market/models/${encodeURIComponent(row.model_key)}`} className="group flex h-full flex-col gap-2 p-4 hover:bg-s-1">
                  <span className="flex min-w-0 items-center gap-2">
                    <Logo name={creatorName(row.maker) ?? row.model_key} src={modelLogoUrl(row.model_key, row.maker)} size="sm" />
                    <span className="truncate font-mono text-[13px] text-ink group-hover:text-s-7">{row.model_key}</span>
                  </span>
                  <span className="font-display text-[28px] leading-8 tracking-[-0.02em] text-ink tabular">
                    {formatPrice(row.min_blended_per_mtok)}
                  </span>
                  <span className="font-mono text-[12px] text-ink-3">
                    per 1M · {row.providers} providers · via {row.cheapest.name}
                  </span>
                </Link>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section className="grid gap-10 lg:grid-cols-[minmax(0,7fr)_minmax(0,5fr)]">
        <div className="flex min-w-0 flex-col gap-4">
          <SectionHeader title="Tracked model APIs" href="/compare/models" linkLabel="All tracked prices" />
          <ModelBoard models={cheapestModels} />
          <p className="font-mono text-[12px] text-ink-3">
            Blended price per 1M tokens, weighing input 3:1 against output. Lowest price per model.
          </p>
        </div>
        <div className="flex min-w-0 flex-col gap-4">
          <SectionHeader title="Most starred engines" href="/compare/open-source" linkLabel="All projects" />
          <ul className="border border-line">
            {(oss.data ?? []).slice(0, 5).map((p, i) => (
              <li key={p.product_slug} className="border-b border-line last:border-0">
                <Link
                  href={`/products/${p.product_slug}`}
                  className="group flex items-center gap-3 px-4 py-3 hover:bg-s-1"
                >
                  <span className="w-5 font-mono text-[12px] text-ink-3 tabular">{i + 1}</span>
                  <Logo name={p.organization.name} src={p.organization.logo_url} size="sm" />
                  <span className="flex-1 truncate text-[15px] group-hover:text-s-7">{p.product_name}</span>
                  <span className="font-mono text-[13px] text-ink tabular">
                    {p.stars != null ? `${(p.stars / 1000).toFixed(1)}K` : "—"}
                  </span>
                </Link>
              </li>
            ))}
          </ul>
        </div>
      </section>

      {news.data && news.data.items.length > 0 && (
        <section className="flex flex-col gap-4">
          <SectionHeader title="Latest news" href="/news" linkLabel={`All ${news.data.total} stories`} />
          <NewsList items={news.data.items} />
        </section>
      )}

      {categories.data && (
        <section className="flex flex-col gap-4">
          <SectionHeader title="Browse the market" href="/categories" linkLabel="All categories" />
          <CategoryGrid categories={categories.data} />
        </section>
      )}

    </div>
  );
}

/** One GPU type: the cheapest clouds' lowest price, as pixel bars scaled to the dearest shown. */
function GpuBoard({ gpu, listings }: { gpu: string; listings: MarketListing[] }) {
  // Lowest price per GPU-hour per provider, the six cheapest.
  const byProvider = Object.values(
    listings.reduce<Record<string, MarketListing>>((acc, l) => {
      const seen = acc[l.provider.slug];
      if (l.price_per_gpu_hour != null && (!seen || (seen.price_per_gpu_hour ?? Infinity) > l.price_per_gpu_hour))
        acc[l.provider.slug] = l;
      return acc;
    }, {}),
  ).sort((a, b) => (a.price_per_gpu_hour ?? 0) - (b.price_per_gpu_hour ?? 0));
  const shown = byProvider.slice(0, 6);
  const max = Math.max(...shown.map((p) => p.price_per_gpu_hour ?? 0), 0);

  return (
    <div className="border-b border-line px-4 py-4 last:border-0">
      <div className="mb-3 flex items-baseline justify-between">
        <Link
          href={`/market/gpus/${encodeURIComponent(gpu)}`}
          className="font-display text-[18px] tracking-[-0.01em] text-ink hover:text-s-7 hover:underline"
        >
          {gpu}
        </Link>
        <span className="font-mono text-[12px] text-ink-3">
          {byProvider.length > shown.length ? `cheapest ${shown.length} of ${byProvider.length} clouds` : `${byProvider.length} clouds`}
        </span>
      </div>
      {shown.length === 0 ? (
        <p className="font-mono text-[12px] text-ink-3">No prices yet.</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {shown.map((p, i) => (
            <li
              key={p.provider.slug}
              className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 gap-y-1 sm:grid-cols-[132px_minmax(0,1fr)_72px]"
            >
              <Link
                href={`/organizations/${p.provider.slug}`}
                className="flex min-w-0 items-center gap-2 text-[13px] text-ink hover:text-s-7"
              >
                <Logo name={p.provider.name} src={p.provider.logo_url} size="xs" />
                <span className="truncate">{p.provider.name}</span>
              </Link>
              <span className="col-span-2 row-start-2 overflow-hidden sm:col-span-1 sm:col-start-2 sm:row-start-1">
                <PixelBar
                  value={p.price_per_gpu_hour ?? 0}
                  max={max}
                  tone={i === 0 ? "lead" : "muted"}
                  label={`${p.provider.name}: ${formatPrice(p.price_per_gpu_hour)} per GPU-hour`}
                />
              </span>
              <span className={`text-right font-mono text-[13px] tabular ${i === 0 ? "font-medium text-s-7" : "text-ink-2"}`}>
                {formatPrice(p.price_per_gpu_hour)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** The cheapest models as a pixel-bar list. */
function ModelBoard({ models }: { models: ModelPrice[] }) {
  const max = Math.max(...models.map((m) => m.blended_per_mtok ?? 0), 0);
  return (
    <ul className="border border-line">
      {models.map((m, i) => (
        <li key={m.product_slug} className="border-b border-line last:border-0">
          <Link
            href={`/products/${m.product_slug}`}
            className="group grid grid-cols-[28px_minmax(0,1fr)_auto] items-center gap-x-3 gap-y-2 px-4 py-3 hover:bg-s-1 md:grid-cols-[28px_minmax(0,1fr)_minmax(0,160px)_88px]"
          >
            <Logo name={m.maker.name} src={m.maker.logo_url} size="md" />
            <span className="min-w-0">
              <span className="block truncate text-[15px] text-ink group-hover:text-s-7">{m.product_name}</span>
              <span className="block truncate font-mono text-[12px] text-ink-3">
                via {m.provider.name}
                {m.open_weights && " / open weights"}
              </span>
            </span>
            <span className="col-span-3 row-start-2 overflow-hidden md:col-span-1 md:col-start-3 md:row-start-1">
              <PixelBar
                value={m.blended_per_mtok ?? 0}
                max={max}
                cells={20}
                tone={i === 0 ? "lead" : "muted"}
                label={`${m.product_name}: ${formatPrice(m.blended_per_mtok, m.currency)} per million tokens`}
              />
            </span>
            <span
              className={`text-right font-mono text-[14px] tabular ${i === 0 ? "font-medium text-s-7" : "text-ink"}`}
            >
              {formatPrice(m.blended_per_mtok, m.currency)}
            </span>
          </Link>
        </li>
      ))}
    </ul>
  );
}
