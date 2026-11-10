import type { Metadata } from "next";
import Link from "next/link";
import { PageHeader } from "@/components/ui/primitives";

export const metadata: Metadata = {
  title: "Compare",
  description: "Compare model API prices, GPU cloud prices and open-source projects.",
  alternates: { canonical: "/compare" },
};

const PAGES = [
  {
    href: "/compare/models",
    title: "Model API prices",
    body: "Input, output and blended token prices of each model at each provider and gateway.",
  },
  {
    href: "/compare/gpus",
    title: "GPU cloud prices",
    body: "On-demand prices per GPU-hour across clouds, for H100, H200, B200 and more.",
  },
  {
    href: "/compare/open-source",
    title: "Open-source projects",
    body: "Inference engines and runtimes by GitHub stars, releases and recent activity.",
  },
];

export default function ComparePage() {
  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Compare" lead="Side-by-side views of the market, refreshed from providers every few hours." />
      <div className="grid gap-3 sm:grid-cols-3">
        {PAGES.map((page) => (
          <Link
            key={page.href}
            href={page.href}
            className="group flex flex-col gap-1.5 rounded-[10px] bg-surface p-5 shadow-elev-1 transition-shadow hover:shadow-elev-2"
          >
            <h2 className="font-display font-semibold tracking-[-0.01em] text-ink group-hover:text-accent">
              {page.title}
            </h2>
            <p className="text-sm text-ink-2">{page.body}</p>
          </Link>
        ))}
      </div>
    </div>
  );
}
