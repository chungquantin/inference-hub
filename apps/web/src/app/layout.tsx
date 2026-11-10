import type { Metadata } from "next";
import { Geist_Mono, Inter, Inter_Tight } from "next/font/google";
import Link from "next/link";
import { MainNav } from "@/components/shell/main-nav";
import { THEME_SCRIPT, ThemeToggle } from "@/components/shell/theme-toggle";
import { SITE_NAME, SITE_URL } from "@/lib/site";
import "./globals.css";

const inter = Inter({ variable: "--font-inter", subsets: ["latin"] });
const interTight = Inter_Tight({
  variable: "--font-inter-tight",
  subsets: ["latin"],
  weight: ["500", "600"],
});
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

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html
      lang="en"
      className={`${inter.variable} ${interTight.variable} ${geistMono.variable}`}
      suppressHydrationWarning
    >
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_SCRIPT }} />
      </head>
      <body className="flex min-h-screen flex-col font-sans">
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-[6px] focus:bg-surface focus:px-3 focus:py-2 focus:shadow-elev-2"
        >
          Skip to content
        </a>
        <header className="sticky top-0 z-40 border-b border-line bg-canvas/85 backdrop-blur-md">
          <div className="mx-auto flex h-14 max-w-[1120px] items-center gap-6 px-6">
            <Link href="/" className="flex shrink-0 items-center gap-2.5">
              <BrandMark />
              <span className="font-display text-[15px] font-semibold tracking-[-0.01em]">
                {SITE_NAME}
              </span>
            </Link>
            <MainNav />
            <div className="ml-auto">
              <ThemeToggle />
            </div>
          </div>
        </header>
        <main id="main" className="mx-auto w-full max-w-[1120px] flex-1 px-6 py-10">
          {children}
        </main>
        <footer className="border-t border-line">
          <div className="mx-auto flex max-w-[1120px] flex-wrap items-center justify-between gap-4 px-6 py-6 text-sm text-ink-3">
            <p>
              Prices come from providers&apos; public APIs and pages, and from curated data. Check
              with the provider before you buy.
            </p>
            <nav aria-label="Footer" className="flex gap-4">
              <Link href="/compare" className="hover:text-ink">
                Compare
              </Link>
              <Link href="/news" className="hover:text-ink">
                News
              </Link>
              <a href="https://github.com/chungquantin/inference-hub" className="hover:text-ink">
                GitHub
              </a>
            </nav>
          </div>
        </footer>
      </body>
    </html>
  );
}

function BrandMark() {
  return (
    <svg width="24" height="24" viewBox="0 0 32 32" aria-hidden>
      <rect width="32" height="32" rx="8" className="fill-accent" />
      <rect x="7" y="16" width="4" height="9" rx="2" className="fill-on-accent" />
      <rect x="14" y="11" width="4" height="14" rx="2" className="fill-on-accent" />
      <rect x="21" y="6" width="4" height="19" rx="2" className="fill-on-accent" fillOpacity=".72" />
    </svg>
  );
}
