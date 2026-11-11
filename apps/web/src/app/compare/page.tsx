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
      <PageHeader
        label="Compare"
        title="Compare the market"
        lead="Side-by-side views of the market, refreshed from providers every few hours."
      />
      <div className="grid grid-cols-1 pt-px pl-px sm:grid-cols-3">
        {PAGES.map((page, i) => (
          <Link
            key={page.href}
            href={page.href}
            className="group -mt-px -ml-px flex flex-col gap-3 border border-line p-6 hover:z-10 hover:border-[color:var(--s-7)]"
          >
            <span className="font-mono text-[13px] text-s-7 tabular">0{i + 1}</span>
            <h2 className="font-display text-[24px] leading-8 tracking-[-0.02em] text-ink group-hover:text-s-7">
              {page.title}
            </h2>
            <p className="text-[14px] leading-6 text-ink-2">{page.body}</p>
            <span className="mt-auto pt-2 font-mono text-[13px] text-s-7 underline">Open</span>
          </Link>
        ))}
      </div>
    </div>
  );
}
