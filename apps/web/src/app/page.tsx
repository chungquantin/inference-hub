import Link from "next/link";
import type { components } from "@inference-hub/api-client";
import { CheapestTable, type CheapTab } from "@/components/home/cheapest-table";
import { Hero } from "@/components/home/hero";
import { api } from "@/lib/api";
import { formatCount, formatPrice } from "@/lib/format";

type MarketListing = components["schemas"]["MarketListing"];

export const revalidate = 300; // seconds; must be a literal for Next.js

/** Model makers above the card, and the clouds that run their models below it. */
const MAKERS = [
  { name: "Anthropic", src: "/logos/anthropic.svg" },
  { name: "OpenAI", src: "/logos/openai.svg" },
  { name: "DeepSeek", src: "/logos/deepseek.png" },
];
const CLOUDS = [
  { name: "RunPod", src: "/logos/runpod.png" },
  { name: "CoreWeave", src: "/logos/coreweave.png" },
  { name: "Together AI", src: "/logos/together-ai.png" },
];

const FAQ = [
  {
    q: "Where do the prices come from?",
    a: "From each provider's public API or price page. Where a provider publishes no machine-readable prices, we use curated data and link the source.",
  },
  {
    q: "How fresh are they?",
    a: "Marketplaces refresh hourly and price lists daily. Every page shows when it was last checked.",
  },
  {
    q: "What is a blended price?",
    a: "One number for a model: three parts input price to one part output price. Use the input and output columns when your mix is different.",
  },
  {
    q: "Why do marketplace GPU prices look so low?",
    a: "Marketplaces such as Vast.ai and Akash list individual hosts. We show the median of their offers, which can differ from a cloud's fixed price. Check availability before you commit.",
  },
  {
    q: "Can I buy through you?",
    a: "No. Inference Hub only compares. You buy from the provider, so confirm the price there first.",
  },
];

export default async function HomePage() {
  const [market, popular, orgs] = await Promise.all([
    api.GET("/v1/market"),
    // The most widely sold models: what most people compare.
    api.GET("/v1/market/models", { params: { query: { min_providers: 8, sort: "price", limit: 6 } } }),
    api.GET("/v1/organizations", { params: { query: { limit: 1 } } }),
  ]);
  const m = market.data;
  const gpuIndex = m?.gpu_index ?? [];

  // The band: the most widely rented GPU, every cloud's lowest price.
  const lead = gpuIndex[0];
  const leadListings = lead
    ? await api.GET("/v1/market/gpus/{gpu_type}", { params: { path: { gpu_type: lead.gpu_type } } })
    : null;
  const points = lead ? cheapestPerProvider(leadListings?.data?.listings ?? []) : [];

  // Real prices for the hero texture.
  const prices = [
    ...gpuIndex.flatMap((g) => [g.min_per_gpu_hour, g.median_per_gpu_hour, g.max_per_gpu_hour].map((n) => n.toFixed(2))),
    ...(m?.model_index ?? []).flatMap((x) => [x.min_blended_per_mtok.toFixed(4), x.median_blended_per_mtok.toFixed(3)]),
  ];

  const tabs: CheapTab[] = [
    {
      id: "gpus",
      label: "GPUs",
      unit: "per GPU-hour",
      total: m ? `${m.gpu_types} GPU types tracked` : "",
      allHref: "/directory?tab=gpus",
      allLabel: "See all GPU prices",
      rows: gpuIndex
        .filter((g) => g.providers >= 5)
        .sort((a, b) => a.min_per_gpu_hour - b.min_per_gpu_hour)
        .slice(0, 6)
        .map((g) => ({
          name: g.gpu_type,
          href: `/market/gpus/${encodeURIComponent(g.gpu_type)}`,
          lo: formatPrice(g.min_per_gpu_hour),
          median: formatPrice(g.median_per_gpu_hour),
          at: g.cheapest.name,
        })),
    },
    {
      id: "models",
      label: "Models",
      unit: "per 1M tokens, blended",
      total: m ? `${formatCount(m.distinct_models)} models tracked` : "",
      allHref: "/directory?tab=models",
      allLabel: "See all model prices",
      rows: (popular.data?.items ?? []).map((x) => ({
        name: x.model_key,
        href: `/market/models/${encodeURIComponent(x.model_key)}`,
        lo: formatPrice(x.min_blended_per_mtok),
        median: formatPrice(x.median_blended_per_mtok),
        at: x.cheapest.name,
      })),
    },
  ];

  const low = points[0];
  const high = points.at(-1);
  const t = (p: number) => (low && high && high.price > low.price ? (Math.log(p) - Math.log(low.price)) / (Math.log(high.price) - Math.log(low.price)) : 0);

  return (
    <div className="flex flex-col">
      <Hero providers={m?.providers ?? 0} prices={prices} makers={MAKERS} clouds={CLOUDS} />

      {lead && low && high && (
        <section aria-labelledby="band-title" className="relative mt-14 grid overflow-hidden border border-line bg-s-2 lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]">
          <div
            aria-hidden
            className="absolute inset-0 opacity-70 [background-image:radial-gradient(circle,#9fb2ee_1.1px,transparent_1.3px)] [background-size:11px_11px] [mask-image:linear-gradient(90deg,transparent_20%,#000)]"
          />
          <div className="relative flex flex-col items-start gap-3.5 bg-charcoal p-8 text-[#f3f1ea] sm:p-9 lg:mt-14 lg:self-end">
            <h2 id="band-title" className="text-[clamp(28px,3.4vw,38px)] leading-[1.08] font-medium tracking-[-0.04em]">
              {lead.gpu_type} from {formatPrice(low.price)} an hour
            </h2>
            <p className="max-w-[38ch] text-[#c9c6bb]">
              {points.length} clouds rent it, from {formatPrice(low.price)} at {low.name} to {formatPrice(high.price)} at {high.name}. The dearest costs{" "}
              {(high.price / low.price).toFixed(1)}× the cheapest.
            </p>
            <Link
              href={`/market/gpus/${encodeURIComponent(lead.gpu_type)}`}
              className="mt-1.5 inline-flex h-[42px] items-center bg-s-7 px-5 text-[15px] font-medium text-wash hover:bg-s-6"
            >
              Compare every cloud
            </Link>
          </div>
          <div className="relative flex flex-col justify-center gap-2.5 p-8 sm:p-10">
            <span className="text-[14px] text-ink-2">Price per GPU-hour, every cloud</span>
            <ul className="relative h-[54px]" aria-label={`${lead.gpu_type} price at each cloud`}>
              <span aria-hidden className="absolute inset-x-0 top-[26px] h-px bg-[#8fa3e6]" />
              {points.map((p) => (
                <li key={p.slug} className="group absolute top-[26px]" style={{ left: `${t(p.price) * 100}%` }}>
                  <Link
                    href={`/organizations/${p.slug}`}
                    aria-label={`${p.name}: ${formatPrice(p.price)} per GPU-hour`}
                    className="relative -mt-2 -ml-2 block size-4 rounded-full border-2 border-s-2 bg-s-7 transition-transform hover:z-10 hover:scale-150 focus-visible:z-10 focus-visible:scale-150"
                  >
                    <span className="pointer-events-none absolute bottom-[22px] left-1/2 -translate-x-1/2 bg-charcoal px-2 py-0.5 text-[12px] whitespace-nowrap text-wash opacity-0 group-focus-within:opacity-100 group-hover:opacity-100">
                      {p.name} {formatPrice(p.price)}
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
            <div className="flex justify-between text-[14px]">
              <span>
                <b className="block font-mono text-[34px] leading-[1.1] font-medium tracking-[-0.04em]">{formatPrice(low.price)}</b>
                <span className="text-ink-2">{low.name}</span>
              </span>
              <span className="text-right">
                <b className="block font-mono text-[34px] leading-[1.1] font-medium tracking-[-0.04em]">{formatPrice(high.price)}</b>
                <span className="text-ink-2">{high.name}</span>
              </span>
            </div>
          </div>
        </section>
      )}

      <Section id="why" title="Built for finding the right price" lead="One place to check what a model or a GPU costs, from the people who sell it.">
        <div className="mt-12 grid border-t border-line-strong sm:grid-cols-2 lg:grid-cols-4">
          <Fact n={m ? formatCount(m.distinct_models) : "—"} title="Models, priced" text="Chat, image, video and speech models, across every provider that sells them." />
          <Fact n={m?.gpu_types ?? "—"} title="GPUs, per hour" text="Each type normalized to one GPU, so a node of eight never skews the number." />
          <Fact n={m?.sources.length ?? "—"} title="Sources read" text="Provider APIs and public price pages, checked every hour or day." />
          <Fact n={orgs.data?.total ?? "—"} title="Companies mapped" text="Labs, clouds, gateways, hardware makers and the investors behind them." />
        </div>
      </Section>

      <section aria-labelledby="now-title" className="grid items-start gap-10 pt-28 lg:grid-cols-[minmax(0,4fr)_minmax(0,7fr)] lg:gap-16">
        <Head id="now-title" title="Cheapest right now" lead="The lowest price we have seen for the most widely sold GPUs and models. Open any row to see every provider." />
        <CheapestTable tabs={tabs} />
      </section>

      <Section id="start" title="Where do you want to start?">
        <ul className="mt-12 border-t border-line-strong">
          <PathRow href="/directory?tab=models" title="Pick a model" text="See who sells it, and what it costs at each provider." cta="Model prices" />
          <PathRow href="/directory?tab=gpus" title="Rent a GPU" text="Hourly prices by GPU type, across clouds and marketplaces." cta="GPU prices" />
          <PathRow href="/directory?tab=orgs" title="Research a company" text="What they sell, who backs them, and what they charge." cta="Companies" />
        </ul>
      </Section>

      <section aria-labelledby="faq-title" className="grid gap-10 pt-28 lg:grid-cols-[minmax(0,4fr)_minmax(0,7fr)] lg:gap-16">
        <Head id="faq-title" title="Questions people ask first" lead="Where the numbers come from, and how far to trust them." />
        <div className="border-t border-line-strong">
          {FAQ.map((f) => (
            <details key={f.q} className="group border-b border-line">
              <summary className="flex cursor-pointer list-none items-center justify-between gap-5 py-5 text-[19px] font-medium tracking-[-0.02em] [&::-webkit-details-marker]:hidden">
                {f.q}
                <span aria-hidden className="text-[24px] leading-none text-ink-3 transition-transform group-open:rotate-45">
                  +
                </span>
              </summary>
              <p className="max-w-[62ch] pb-6 text-ink-2">{f.a}</p>
            </details>
          ))}
        </div>
      </section>

      <section aria-labelledby="cta-title" className="mt-28 border border-line-strong bg-paper px-6 py-16 text-center">
        <h2 id="cta-title" className="mb-2.5 text-[clamp(30px,4vw,44px)] leading-[1.06] font-medium tracking-[-0.045em]">
          Check the price before you buy.
        </h2>
        <p className="mx-auto mb-7 max-w-[46ch] text-ink-2">Search a model or a GPU and see every provider in one table.</p>
        <div className="flex flex-wrap justify-center gap-3">
          <Link href="/directory" className="inline-flex h-[42px] items-center bg-s-7 px-5 text-[15px] font-medium text-wash hover:bg-s-8">
            Open the directory
          </Link>
          <Link
            href="/market/providers"
            className="inline-flex h-[42px] items-center px-5 text-[15px] font-medium shadow-[inset_0_0_0_1px_var(--line-strong)] hover:shadow-[inset_0_0_0_1px_var(--ink)]"
          >
            See how providers compare
          </Link>
        </div>
      </section>
    </div>
  );
}

function Head({ id, title, lead }: { id: string; title: string; lead?: string }) {
  return (
    <div className="max-w-[560px]">
      <h2 id={id} className="mb-3 text-[clamp(30px,4vw,44px)] leading-[1.06] font-medium tracking-[-0.045em]">
        {title}
      </h2>
      {lead && <p className="max-w-[46ch] text-[18px] leading-[1.55] text-ink-2">{lead}</p>}
    </div>
  );
}

function Section({ id, title, lead, children }: { id: string; title: string; lead?: string; children: React.ReactNode }) {
  return (
    <section aria-labelledby={`${id}-title`} className="pt-28">
      <Head id={`${id}-title`} title={title} lead={lead} />
      {children}
    </section>
  );
}

function Fact({ n, title, text }: { n: React.ReactNode; title: string; text: string }) {
  return (
    <div className="border-line pt-6 pr-6 pb-2 sm:pl-6 sm:[&:nth-child(2n+1)]:border-l-0 sm:[&:nth-child(2n+1)]:pl-0 lg:[&:nth-child(n+2)]:border-l lg:[&:nth-child(n+2)]:pl-6 lg:[&:nth-child(2n+1)]:border-l">
      <span className="mb-2.5 block font-mono text-[34px] leading-[1.1] font-medium tracking-[-0.04em] text-s-7">{n}</span>
      <h3 className="mb-1.5 text-[18px] font-semibold tracking-[-0.02em]">{title}</h3>
      <p className="max-w-[30ch] text-[15px] text-ink-2">{text}</p>
    </div>
  );
}

function PathRow({ href, title, text, cta }: { href: string; title: string; text: string; cta: string }) {
  return (
    <li className="border-b border-line">
      <Link
        href={href}
        className="grid items-baseline gap-x-6 gap-y-1 py-6 hover:bg-paper md:grid-cols-[minmax(0,4fr)_minmax(0,6fr)_120px] md:hover:-mx-3.5 md:hover:px-3.5"
      >
        <span className="text-[clamp(22px,2.6vw,30px)] font-medium tracking-[-0.04em]">{title}</span>
        <span className="text-ink-2">{text}</span>
        <span className="text-[15px] text-s-7 md:text-right">{cta}</span>
      </Link>
    </li>
  );
}

/** Each cloud's lowest price per GPU-hour for one GPU type, cheapest first. */
function cheapestPerProvider(listings: MarketListing[]): { slug: string; name: string; price: number }[] {
  const by: Record<string, MarketListing> = {};
  for (const l of listings) {
    const seen = by[l.provider.slug];
    if (l.price_per_gpu_hour != null && (!seen || (seen.price_per_gpu_hour ?? Infinity) > l.price_per_gpu_hour)) by[l.provider.slug] = l;
  }
  return Object.values(by)
    .map((l) => ({ slug: l.provider.slug, name: l.provider.name, price: l.price_per_gpu_hour ?? 0 }))
    .filter((p) => p.price > 0)
    .sort((a, b) => a.price - b.price);
}
