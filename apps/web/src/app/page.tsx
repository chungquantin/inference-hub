import Link from "next/link";
import type { GpuPrice, ModelPrice } from "@inference-hub/api-client";
import { CategoryGrid } from "@/components/category-grid";
import { SearchForm } from "@/components/listing";
import { NewsList } from "@/components/news";
import { Logo } from "@/components/ui/logo";
import { SectionHeader } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { formatDate, formatPrice } from "@/lib/format";

export const revalidate = 300; // seconds; must be a literal for Next.js

/** GPU types shown on the price board, in order. */
const BOARD_GPUS = ["H100 SXM", "H200 SXM", "B200"];

export default async function HomePage() {
  const [gpus, models, categories, news, orgs] = await Promise.all([
    api.GET("/v1/compare/gpus"),
    api.GET("/v1/compare/models", { params: { query: { sort: "blended" } } }),
    api.GET("/v1/categories"),
    api.GET("/v1/news", { params: { query: { limit: 6 } } }),
    api.GET("/v1/organizations", { params: { query: { limit: 1 } } }),
  ]);

  // Cheapest per GPU-hour for each board GPU, and the number of clouds pricing it.
  const gpuBoard = BOARD_GPUS.flatMap((gpu) => {
    const rows = (gpus.data?.prices ?? []).filter((p) => p.gpu_type === gpu);
    if (rows.length === 0) return [];
    const providers = new Set(rows.map((r) => r.provider.slug)).size;
    return [{ gpu, best: rows[0], providers }];
  });
  // Cheapest open-weights and overall model by blended price.
  const modelRows = models.data ?? [];
  const cheapestOpen = modelRows.find((m) => m.open_weights && (m.blended_per_mtok ?? 0) > 0);
  const priced = new Set(modelRows.map((m) => m.product_slug)).size;

  return (
    <div className="flex flex-col gap-14">
      <section className="grid items-start gap-8 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.15fr)]">
        <div className="flex min-w-0 flex-col gap-5 pt-2">
          <h1 className="font-display text-[2rem] leading-[1.1] font-semibold tracking-[-0.025em] text-ink sm:text-[2.6rem] sm:leading-[1.08]">
            What inference costs today, and who sells it
          </h1>
          <p className="max-w-[52ch] text-[15px] leading-relaxed text-ink-2">
            Model API and GPU prices collected from providers, next to the companies, open-source
            projects and investors behind them.
          </p>
          <SearchForm action="/organizations" placeholder="Search companies and projects" />
          <p className="text-sm text-ink-3">
            {orgs.data?.total ?? 0} organizations, {priced} models priced across providers.
          </p>
        </div>

        <div className="flex min-w-0 flex-col gap-3">
          <div className="overflow-hidden rounded-[10px] bg-surface shadow-elev-2">
            <div className="flex items-baseline justify-between border-b border-line px-4 py-3">
              <h2 className="font-display text-[15px] font-semibold text-ink">
                Cheapest GPU per hour
              </h2>
              <Link href="/compare/gpus" className="text-sm text-ink-2 hover:text-accent">
                All GPU prices
              </Link>
            </div>
            <ul>
              {gpuBoard.map(({ gpu, best, providers }) => (
                <GpuBoardRow key={gpu} gpu={gpu} best={best} providers={providers} />
              ))}
              {gpuBoard.length === 0 && (
                <li className="px-4 py-6 text-sm text-ink-3">No GPU prices collected yet.</li>
              )}
            </ul>
          </div>
          {cheapestOpen && <ModelBoard model={cheapestOpen} />}
        </div>
      </section>

      {news.data && news.data.items.length > 0 && (
        <section className="flex flex-col gap-3">
          <SectionHeader title="Latest news" href="/news" linkLabel={`All ${news.data.total} stories`} />
          <NewsList items={news.data.items} />
        </section>
      )}

      {categories.data && (
        <section className="flex flex-col gap-3">
          <SectionHeader title="Browse the market" href="/categories" linkLabel="All categories" />
          <CategoryGrid categories={categories.data} />
        </section>
      )}
    </div>
  );
}

function GpuBoardRow({ gpu, best, providers }: { gpu: string; best: GpuPrice; providers: number }) {
  return (
    <li className="border-b border-line last:border-0">
      <Link
        href={`/compare/gpus?gpu=${encodeURIComponent(gpu)}`}
        className="grid grid-cols-[1fr_auto] items-center gap-x-4 gap-y-1 px-4 py-3.5 hover:bg-hover/50 sm:grid-cols-[7.5rem_1fr_auto]"
      >
        <span className="font-display font-semibold text-ink">{gpu}</span>
        <span className="col-start-1 row-start-2 flex items-center gap-2 text-sm text-ink-2 sm:col-start-2 sm:row-start-1">
          <Logo name={best.provider.name} src={best.provider.logo_url} size="sm" />
          {best.provider.name}
          <span className="text-ink-3">
            · {providers} {providers === 1 ? "cloud" : "clouds"}
          </span>
        </span>
        <span className="row-span-2 text-right sm:row-span-1">
          <span className="font-mono text-xl font-medium tabular-nums text-ink">
            {formatPrice(best.price_per_gpu_hour, best.currency)}
          </span>
          <span className="block text-xs text-ink-3">per GPU-hour · {formatDate(best.observed_at)}</span>
        </span>
      </Link>
    </li>
  );
}

function ModelBoard({ model }: { model: ModelPrice }) {
  return (
    <Link
      href="/compare/models?open=1"
      className="flex items-center gap-3 rounded-[10px] bg-surface px-4 py-3.5 shadow-elev-1 hover:shadow-elev-2"
    >
      <Logo name={model.maker.name} src={model.maker.logo_url} size="md" />
      <span className="min-w-0 flex-1">
        <span className="block text-xs text-ink-3">Cheapest open-weights model API</span>
        <span className="block truncate font-medium text-ink">
          {model.product_name} <span className="font-normal text-ink-3">via {model.provider.name}</span>
        </span>
      </span>
      <span className="text-right">
        <span className="font-mono text-lg font-medium tabular-nums text-ink">
          {formatPrice(model.blended_per_mtok, model.currency)}
        </span>
        <span className="block text-xs text-ink-3">per 1M tokens, blended</span>
      </span>
    </Link>
  );
}
