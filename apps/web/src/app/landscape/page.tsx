import type { Metadata } from "next";
import { LandscapeData } from "@/components/landscape/landscape-data";
import { PageHeader } from "@/components/ui/primitives";

export const revalidate = 300;

export const metadata: Metadata = {
  title: "The inference market landscape",
  description:
    "Every lab, inference provider, gateway, GPU cloud, chip maker, agent and investor in the inference market on one page, with valuation and funding where we have a source.",
  alternates: { canonical: "/landscape" },
};

export default function LandscapePage() {
  return (
    <div data-wide className="flex flex-col gap-8">
      <PageHeader
        title="The landscape"
        lead="Everyone in the inference market, grouped by what they do. Open a company for what it does and who funded it. Funding appears only where we could link a source."
      />
      <LandscapeData />
    </div>
  );
}
