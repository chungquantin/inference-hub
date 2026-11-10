import type { Metadata } from "next";
import Link from "next/link";

export const metadata: Metadata = {
  title: "Compare",
  description: "Compare model API prices, GPU cloud prices and open-source projects.",
  alternates: { canonical: "/compare" },
};

const PAGES = [
  {
    href: "/compare/models",
    title: "Model API prices",
    body: "Input, output and blended token prices of each model at each provider.",
  },
  {
    href: "/compare/gpus",
    title: "GPU cloud prices",
    body: "Hourly and per-GPU prices of GPU instances across clouds.",
  },
  {
    href: "/compare/open-source",
    title: "Open-source projects",
    body: "Inference engines and runtimes by GitHub stars and momentum.",
  },
];

export default function ComparePage() {
  return (
    <div className="flex flex-col gap-6">
      <h1 className="text-3xl font-semibold tracking-tight">Compare</h1>
      <div className="grid gap-4 sm:grid-cols-3">
        {PAGES.map((page) => (
          <Link
            key={page.href}
            href={page.href}
            className="group rounded-lg border border-zinc-200 p-4 transition-colors hover:border-zinc-400 dark:border-zinc-800 dark:hover:border-zinc-600"
          >
            <h2 className="font-medium group-hover:underline">{page.title}</h2>
            <p className="mt-1 text-sm text-zinc-600 dark:text-zinc-400">{page.body}</p>
          </Link>
        ))}
      </div>
    </div>
  );
}
