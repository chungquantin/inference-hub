import Link from "next/link";
import type { GpuPrice, ModelPrice } from "@inference-hub/api-client";
import { CategoryGrid } from "@/components/category-grid";
import { SearchForm } from "@/components/listing";
import { NewsList } from "@/components/news";
import { Logo } from "@/components/ui/logo";
import { LiveDot, PixelBar, PixelBullet, WindowFrame } from "@/components/ui/pixel";
import { SectionHeader, Stat, buttonClass } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatDate, formatPrice } from "@/lib/format";

export const revalidate = 300; // seconds; must be a literal for Next.js

/** GPU types on the board, in order. */
const BOARD_GPUS = ["H100 SXM", "H200 SXM", "B200"];

export default async function HomePage() {
  const [gpus, models, categories, news, orgs, oss] = await Promise.all([
    api.GET("/v1/compare/gpus"),
    api.GET("/v1/compare/models", { params: { query: { sort: "blended" } } }),
    api.GET("/v1/categories"),
    api.GET("/v1/news", { params: { query: { limit: 6 } } }),
    api.GET("/v1/organizations", { params: { query: { limit: 1 } } }),
    api.GET("/v1/compare/open-source"),
  ]);

  const gpuPrices = gpus.data?.prices ?? [];
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
  const latest = [...gpuPrices.map((p) => p.observed_at), ...modelPrices.map((p) => p.observed_at)]
    .sort()
    .at(-1);
  const providers = new Set([...gpuPrices, ...modelPrices].map((p) => p.provider.slug)).size;

  return (
    <div className="flex flex-col gap-20">
      <section className="grid gap-10 lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]">
        <div className="flex min-w-0 flex-col gap-6">
          <p className="flex items-center gap-2 font-mono text-[13px] text-s-7">
            <LiveDot />
            {latest ? `Prices checked ${formatDate(latest)}` : "Market prices"}
          </p>
          <h1 className="font-display text-[40px] leading-[44px] tracking-[-0.03em] text-ink sm:text-[56px] sm:leading-[60px]">
            What inference costs, and who sells it.
          </h1>
          <p className="max-w-[46ch] text-[17px] leading-7 text-ink-2">
            Model API and GPU prices collected from the providers themselves, next to the
            companies, open-source projects and investors of the market.
          </p>
          <div className="flex flex-wrap gap-3">
            <Link href="/compare/gpus" className={buttonClass("primary")}>
              Compare GPU prices
            </Link>
            <Link href="/compare/models" className={buttonClass("secondary")}>
              Compare model APIs
            </Link>
          </div>
          <ul className="mt-2 flex flex-col gap-2 text-[14px] text-ink-2">
            {[
              "Refreshed from OpenRouter, Liquid, RunPod, Hyperbolic and CoreWeave",
              "Every price links to where it was read",
              "History kept for every change",
            ].map((line) => (
              <li key={line} className="flex items-center gap-3">
                <PixelBullet />
                {line}
              </li>
            ))}
          </ul>
        </div>

        <WindowFrame
          title="GPU price board / per GPU-hour"
          meta={<Link href="/compare/gpus" className="hover:text-wash hover:underline">all prices</Link>}
          className="min-w-0"
        >
          {BOARD_GPUS.map((gpu) => (
            <GpuBoard key={gpu} gpu={gpu} prices={gpuPrices.filter((p) => p.gpu_type === gpu)} />
          ))}
        </WindowFrame>
      </section>

      <section className="grid grid-cols-2 pt-px pl-px lg:grid-cols-4 [&>*]:-mt-px [&>*]:-ml-px [&>*]:border [&>*]:border-line">
        <Stat value={orgs.data?.total ?? "—"} label="Organizations tracked" href="/organizations" />
        <Stat value={providers} label="Providers with live prices" href="/compare" />
        <Stat
          value={modelPrices.length + gpuPrices.length}
          label="Prices compared"
          href="/compare/models"
        />
        <Stat value={oss.data?.length ?? "—"} label="Open-source projects" href="/compare/open-source" />
      </section>

      <section className="grid gap-10 lg:grid-cols-[minmax(0,7fr)_minmax(0,5fr)]">
        <div className="flex min-w-0 flex-col gap-4">
          <SectionHeader title="Cheapest model APIs" href="/compare/models" linkLabel="All model prices" />
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

      <section className="flex flex-col items-start gap-5 border border-ink p-8 sm:p-10">
        <h2 className="font-display text-[32px] leading-10 tracking-[-0.02em]">Find a company or project</h2>
        <SearchForm action="/organizations" placeholder="vLLM, CoreWeave, Mistral..." />
      </section>
    </div>
  );
}

/** One GPU type: every provider's lowest price, as pixel bars scaled to the dearest. */
function GpuBoard({ gpu, prices }: { gpu: string; prices: GpuPrice[] }) {
  // Lowest price per provider for this GPU.
  const byProvider = Object.values(
    prices.reduce<Record<string, GpuPrice>>((acc, p) => {
      const seen = acc[p.provider.slug];
      if (!seen || seen.price_per_gpu_hour > p.price_per_gpu_hour) acc[p.provider.slug] = p;
      return acc;
    }, {}),
  ).sort((a, b) => a.price_per_gpu_hour - b.price_per_gpu_hour);
  const max = Math.max(...byProvider.map((p) => p.price_per_gpu_hour), 0);

  return (
    <div className="border-b border-line px-4 py-4 last:border-0">
      <div className="mb-3 flex items-baseline justify-between">
        <Link
          href={`/compare/gpus?gpu=${encodeURIComponent(gpu)}`}
          className="font-display text-[18px] tracking-[-0.01em] text-ink hover:text-s-7 hover:underline"
        >
          {gpu}
        </Link>
        <span className="font-mono text-[12px] text-ink-3">
          {byProvider.length} {byProvider.length === 1 ? "cloud" : "clouds"}
        </span>
      </div>
      {byProvider.length === 0 ? (
        <p className="font-mono text-[12px] text-ink-3">No prices yet.</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {byProvider.map((p, i) => (
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
                  value={p.price_per_gpu_hour}
                  max={max}
                  tone={i === 0 ? "lead" : "muted"}
                  label={`${p.provider.name}: ${formatPrice(p.price_per_gpu_hour, p.currency)} per GPU-hour`}
                />
              </span>
              <span
                className={`text-right font-mono text-[13px] tabular ${i === 0 ? "font-medium text-s-7" : "text-ink-2"}`}
              >
                {formatPrice(p.price_per_gpu_hour, p.currency)}
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
