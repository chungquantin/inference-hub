import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import Link from "next/link";
import { SITE_NAME, SITE_URL } from "@/lib/site";
import "./globals.css";

const geistSans = Geist({ variable: "--font-geist-sans", subsets: ["latin"] });
const geistMono = Geist_Mono({ variable: "--font-geist-mono", subsets: ["latin"] });

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: {
    default: SITE_NAME,
    template: `%s · ${SITE_NAME}`,
  },
  openGraph: { siteName: SITE_NAME, type: "website" },
  description:
    "The map of the inference market: model providers, LLM gateways, open-source projects, GPU clouds, hyperscalers and investors.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${geistSans.variable} ${geistMono.variable}`}>
      <body className="min-h-screen font-sans">
        <header className="border-b border-zinc-200 dark:border-zinc-800">
          <nav className="mx-auto flex max-w-6xl items-center justify-between px-6 py-4">
            <Link href="/" className="font-semibold tracking-tight">
              Inference Hub
            </Link>
            <div className="flex gap-6 text-sm text-zinc-600 dark:text-zinc-400">
              <Link href="/categories" className="hover:text-zinc-900 dark:hover:text-zinc-100">
                Categories
              </Link>
              <Link href="/organizations" className="hover:text-zinc-900 dark:hover:text-zinc-100">
                Directory
              </Link>
              <Link href="/products" className="hover:text-zinc-900 dark:hover:text-zinc-100">
                Products
              </Link>
              <Link href="/news" className="hover:text-zinc-900 dark:hover:text-zinc-100">
                News
              </Link>
            </div>
          </nav>
        </header>
        <main className="mx-auto max-w-6xl px-6 py-10">{children}</main>
      </body>
    </html>
  );
}
