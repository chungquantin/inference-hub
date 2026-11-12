import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { HistoryPanel, Ladder, Picker, TermChart, TermTable } from "@/components/analytics/curves";
import { Frame, PageHeader } from "@/components/ui/primitives";
import { api } from "@/lib/api";

export const revalidate = 300;

export const metadata: Metadata = {
  title: "Forward curves for GPU and model prices",
  description:
    "What a GPU-hour costs as spot, on demand and committed for a term, by provider, and how GPU and model prices have moved day by day.",
  alternates: { canonical: "/analytics/curves" },
};

type Search = { tab?: string; gpu?: string; model?: string };

function Panel({ title, note, children }: { title: string; note?: React.ReactNode; children: React.ReactNode }) {
  return (
    <Frame as="section" className="flex flex-col">
      <header className="flex flex-wrap items-baseline justify-between gap-2 border-b border-line px-4 py-3">
        <h2 className="font-display text-[19px] leading-6 font-medium tracking-[-0.03em] text-ink">{title}</h2>
        {note && <span className="text-[13px] text-ink-3">{note}</span>}
      </header>
      {children}
    </Frame>
  );
}

export default async function CurvesPage({ searchParams }: { searchParams: Promise<Search> }) {
  const sp = await searchParams;
  const { data: index, error } = await api.GET("/v1/market/curves");
  if (error || !index) throw new Error("Could not load the curves");
  const tab = sp.tab === "models" ? "models" : "gpus";
  // Default to the first GPU with the most tiers (the best term structure) and the most widely sold model.
  const gpuKey = sp.gpu ?? index.gpus[0]?.key;
  const modelKey = sp.model ?? index.models[0]?.key;

  const tabs = (
    <div role="tablist" aria-label="Curve type" className="flex gap-1 border-b border-line">
      {[
        { id: "gpus", label: "GPU-hours" },
        { id: "models", label: "Models" },
      ].map((t) => (
        <Link
          key={t.id}
          role="tab"
          aria-selected={tab === t.id}
          href={`/analytics/curves?tab=${t.id}`}
          replace
          className={`-mb-px inline-flex h-11 items-center border-b-2 px-5 text-[15px] font-medium ${
            tab === t.id ? "border-s-7 text-s-7" : "border-transparent text-ink-2 hover:text-ink"
          }`}
        >
          {t.label}
        </Link>
      ))}
    </div>
  );

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        crumbs={[{ href: "/analytics", label: "Analytics" }, { label: "Forward curves" }]}
        title="Forward curves"
        lead="How the price changes with how you buy. For a GPU-hour: spot, on demand, then one to five year commitments. For a model: every provider’s price from cheapest to dearest. Both with the market’s daily history."
      />
      {tabs}
      {tab === "gpus" ? (
        <GpuView items={index.gpus} gpuKey={gpuKey} />
      ) : (
        <ModelView items={index.models} modelKey={modelKey} />
      )}
    </div>
  );
}

async function GpuView({ items, gpuKey }: { items: Parameters<typeof Picker>[0]["items"]; gpuKey?: string }) {
  if (!gpuKey) return <p className="text-ink-3">No GPU prices yet.</p>;
  const { data, response } = await api.GET("/v1/market/curves/gpus/{gpu_type}", { params: { path: { gpu_type: gpuKey } } });
  if (response.status === 404) notFound();
  if (!data) throw new Error("Could not load the GPU curve");
  return (
    <>
      <Picker label="GPU type" param="gpu" items={items} value={gpuKey} extra={{ tab: "gpus" }} />
      <Panel
        title={`${data.gpu_type}: price per GPU-hour by term`}
        note={`${new Set(data.points.map((p) => p.organization.slug)).size} providers · ${data.nodes.length} price tiers`}
      >
        <TermChart curve={data} />
      </Panel>
      <Panel title="By provider">
        <TermTable curve={data} />
      </Panel>
      <Panel title="Market price history" note="Daily, from the first snapshot">
        <HistoryPanel history={data.history} unit="gpu" hasMax />
      </Panel>
    </>
  );
}

async function ModelView({ items, modelKey }: { items: Parameters<typeof Picker>[0]["items"]; modelKey?: string }) {
  if (!modelKey) return <p className="text-ink-3">No model prices yet.</p>;
  const { data, response } = await api.GET("/v1/market/curves/models/{model_key}", { params: { path: { model_key: modelKey } } });
  if (response.status === 404) notFound();
  if (!data) throw new Error("Could not load the model curve");
  return (
    <>
      <Picker label="Model" param="model" items={items} value={modelKey} extra={{ tab: "models" }} />
      <Panel title={`${data.name}: price per million tokens by provider`} note="3:1 input to output blend, cheapest first">
        <Ladder curve={data} />
      </Panel>
      <Panel title="Market price history" note="Daily, from the first snapshot">
        <HistoryPanel history={data.history} unit="model" hasMax={false} />
      </Panel>
    </>
  );
}
