import type { Metadata } from "next";
import { Geist_Mono, Instrument_Sans } from "next/font/google";
import Link from "next/link";
import { connection } from "next/server";
import { CommandPalette } from "@/components/shell/command-palette";
import { MainNav } from "@/components/shell/main-nav";
import { LiveDot } from "@/components/ui/pixel";
import { api } from "@/lib/api";
import { NAV_SECTIONS } from "@/lib/nav";
import { SITE_NAME, SITE_URL } from "@/lib/site";
import "./globals.css";

const instrument = Instrument_Sans({ variable: "--font-instrument", subsets: ["latin"] });
const geistMono = Geist_Mono({ variable: "--font-geist-mono", subsets: ["latin"] });

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: {
    default: `${SITE_NAME}: prices and providers of the inference market`,
    template: `%s · ${SITE_NAME}`,
  },
  openGraph: { siteName: SITE_NAME, type: "website" },
  description:
    "Compare model API and GPU prices across providers, and follow the companies, projects and investors of the inference market.",
};

export const viewport = { themeColor: "#fcfbf8" };

export default async function RootLayout({ children }: { children: React.ReactNode }) {
  // Render on request: pages read the API, which does not exist at build time.
  await connection();
  return (
    <html
      lang="en"
      className={`${instrument.variable} ${geistMono.variable}`}
    >
      <body className="flex min-h-screen flex-col font-sans">
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:bg-ink focus:px-3 focus:py-2 focus:text-wash"
        >
          Skip to content
        </a>
        <AnnouncementBar />
        <header className="sticky top-0 z-40 border-b border-line bg-ground">
          <div className="mx-auto flex h-16 max-w-(--page-width) items-stretch gap-6 px-4 sm:px-6">
            <Link href="/" className="flex shrink-0 items-center gap-2.5">
              <BrandMark />
              <span className="font-display text-[20px] font-semibold tracking-[-0.045em] text-ink">{SITE_NAME}</span>
            </Link>
            <MainNav />
          </div>
        </header>
        <main id="main" className="mx-auto w-full max-w-(--page-width) flex-1 px-4 pt-10 pb-24 sm:px-6">
          {children}
        </main>
        <Footer />
        <CommandPalette />
      </body>
    </html>
  );
}

/** Three rising bars on a square: prices across providers. The lowest bar is highlighted. */
function BrandMark() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" aria-hidden shapeRendering="crispEdges">
      <rect width="24" height="24" className="fill-ink" />
      <rect x="5" y="12" width="3" height="7" className="fill-wash" />
      <rect x="10.5" y="8" width="3" height="11" className="fill-wash" />
      <rect x="16" y="4" width="3" height="15" className="fill-s-6" />
    </svg>
  );
}

async function Footer() {
  // Live counts for the status bar; the footer still renders if the API is down.
  const [orgs, market, news] = await Promise.all([
    api.GET("/v1/organizations", { params: { query: { limit: 1 } } }).catch(() => null),
    api.GET("/v1/market").catch(() => null),
    api.GET("/v1/news", { params: { query: { limit: 1 } } }).catch(() => null),
  ]);
  const m = market?.data;
  const status = [
    orgs?.data && `${orgs.data.total} organizations`,
    m && `${m.providers} providers`,
    m && `${m.distinct_models.toLocaleString("en-US")} models`,
    m && `${m.gpu_types} GPU types`,
    news?.data && `${news.data.total.toLocaleString("en-US")} stories`,
  ].filter(Boolean);

  return (
    <footer className="bg-charcoal text-wash">
      <div className="mx-auto grid max-w-(--page-width) gap-10 px-6 py-14 md:grid-cols-[minmax(0,4fr)_minmax(0,8fr)]">
        <div className="flex flex-col gap-4">
          <p className="font-display text-[30px] leading-9 font-medium tracking-[-0.045em]">
            The price of inference, from the providers themselves.
          </p>
          <p className="max-w-[48ch] text-[14px] leading-6 text-s-3">
            Prices come from providers&apos; public APIs and pages every few hours, and from curated
            data with sources. Check with the provider before you buy.
          </p>
        </div>
        <div className="grid grid-cols-2 gap-8 sm:grid-cols-4">
          {NAV_SECTIONS.map((section) => (
            <FooterCol key={section.href} title={section.label}>
              {section.items.map((l) => (
                <Link key={l.href} href={l.href}>
                  {l.label}
                </Link>
              ))}
            </FooterCol>
          ))}
          <FooterCol title="Project">
            <a href="https://github.com/chungquantin/inference-hub">GitHub</a>
            <a href="/catalog/NOTICE.md">Model icons: inference-catalog (MIT)</a>
            <a href={`${SITE_URL}/sitemap.xml`}>Sitemap</a>
            {process.env.PUBLIC_API_URL && (
              <a href={`${process.env.PUBLIC_API_URL.replace(/\/$/, "")}/docs`}>API docs</a>
            )}
          </FooterCol>
        </div>
      </div>
      <div className="bg-black/25">
        <div className="mx-auto flex h-10 max-w-(--page-width) items-center gap-5 overflow-x-auto px-6 text-[13px] whitespace-nowrap text-s-3">
          <span className="flex items-center gap-2 font-medium text-wash">
            <LiveDot />
            Live
          </span>
          {status.map((s) => (
            <span key={s as string}>{s}</span>
          ))}
          <span className="ml-auto hidden text-s-4 lg:inline">Marketplaces refresh hourly, price lists daily</span>
        </div>
      </div>
    </footer>
  );
}

function FooterCol({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5 text-[14px] [&_a]:text-s-3 [&_a:hover]:text-wash [&_a:hover]:underline">
      <span className="mb-1 text-[14px] font-semibold text-wash">{title}</span>
      {children}
    </div>
  );
}

/** A slim bar above the header: the one fact that earns trust, with a link to the evidence. */
async function AnnouncementBar() {
  const market = await api.GET("/v1/market").catch(() => null);
  const m = market?.data;
  if (!m) return null;
  return (
    <div className="bg-charcoal px-4 py-2 text-center text-[14px] text-[#f3f1ea]">
      <span className="mr-2.5 bg-[#f0d9a0] px-1.5 py-0.5 text-[11px] font-semibold text-charcoal">NEW</span>
      <Link href="/analytics/providers" className="underline underline-offset-[3px]">
        Prices from {m.providers} providers, refreshed hourly
      </Link>
    </div>
  );
}
