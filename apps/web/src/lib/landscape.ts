import type { LandscapeItem, LandscapeRound } from "@inference-hub/api-client";

/** A band of the map. An organization sits in the first band whose kinds it has. */
export type Group = { id: string; label: string; kinds: string[]; color: string; blurb: string };

export const GROUPS: Group[] = [
  { id: "hyperscalers", label: "Hyperscalers", kinds: ["hyperscaler"], color: "#7a3fb0", blurb: "General-purpose clouds with AI compute" },
  { id: "model-labs", label: "Model labs", kinds: ["model_provider"], color: "#3a5bd9", blurb: "Companies training and serving foundation models" },
  { id: "inference", label: "Inference providers", kinds: ["inference_provider"], color: "#0f7a48", blurb: "Platforms serving models through an API" },
  { id: "gateways", label: "Gateways and routers", kinds: ["llm_gateway"], color: "#0a7396", blurb: "One API in front of many models" },
  { id: "gpu-clouds", label: "GPU clouds", kinds: ["gpu_cloud"], color: "#cc3a21", blurb: "Specialized GPU and accelerator clouds" },
  { id: "decentralized", label: "Decentralized compute", kinds: ["decentralized"], color: "#96620a", blurb: "Networks pooling compute from many operators" },
  { id: "hardware", label: "Chips and hardware", kinds: ["hardware"], color: "#55534b", blurb: "Accelerators, servers and networking" },
  { id: "agents", label: "Agents and harnesses", kinds: ["agent_tool"], color: "#b5439b", blurb: "Coding agents, frameworks and assistants that spend the tokens" },
  { id: "llmops", label: "LLMOps", kinds: ["llmops"], color: "#2c47b3", blurb: "Observability, evaluation and guardrails" },
  { id: "open-source", label: "Open source", kinds: ["open_source"], color: "#5a6b1f", blurb: "Engines, runtimes and serving tools" },
  { id: "investors", label: "Investors", kinds: ["investor"], color: "#223a92", blurb: "Funds and strategics backing the market" },
  { id: "other", label: "Other", kinds: ["other"], color: "#75726a", blurb: "Everything else in the directory" },
];

export function groupOf(item: LandscapeItem): Group {
  return GROUPS.find((g) => g.kinds.some((k) => (item.kinds as string[]).includes(k))) ?? GROUPS[GROUPS.length - 1];
}

/** What a tile says about money: the latest valuation, else what was raised. */
export type Money = { amount: number; currency: string };
export type Headline = { valuation?: Money; raised?: Money; latest?: LandscapeRound };

export function headline(item: LandscapeItem): Headline {
  const rounds = item.rounds; // newest first
  if (rounds.length === 0) return {};
  const latest = rounds[0];
  const withValuation = rounds.find((r) => r.valuation != null);
  // Sum only rounds in the currency of the latest one, so amounts never mix currencies.
  const raised = rounds
    .filter((r) => r.currency === latest.currency && r.amount != null)
    .reduce((s, r) => s + (r.amount ?? 0), 0);
  return {
    latest,
    valuation: withValuation ? { amount: withValuation.valuation as number, currency: withValuation.currency } : undefined,
    raised: raised > 0 ? { amount: raised, currency: latest.currency } : undefined,
  };
}

export function compactMoney(m: Money): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: m.currency,
    notation: "compact",
    maximumFractionDigits: m.amount >= 1e9 ? 1 : 0,
  }).format(m.amount);
}

export type SortMode = "market" | "valuation" | "raised" | "name";

/**
 * `market`: who sells the most at a price in the market first, then the funded. `valuation` and
 * `raised`: funded companies first by that measure, then the busiest sellers. Always ending with
 * those that have a logo, then by name.
 */
export function sortItems(items: LandscapeItem[], mode: SortMode): LandscapeItem[] {
  const funding = (i: LandscapeItem) => {
    const h = headline(i);
    return mode === "raised" ? (h.raised?.amount ?? 0) : (h.valuation?.amount ?? 0) || (h.raised?.amount ?? 0) / 1e3;
  };
  const sells = (i: LandscapeItem) => i.models_sold + i.gpu_configs;
  const primary = (i: LandscapeItem) => (mode === "market" ? sells(i) : mode === "name" ? 0 : funding(i));
  const secondary = (i: LandscapeItem) => (mode === "market" ? funding(i) : sells(i));
  return [...items].sort(
    (a, b) =>
      primary(b) - primary(a) ||
      secondary(b) - secondary(a) ||
      Number(Boolean(b.logo_url)) - Number(Boolean(a.logo_url)) ||
      a.name.localeCompare(b.name),
  );
}

/** Whether the organization has a price in the market: chat models or GPU configurations. */
export const sellsOnMarket = (i: LandscapeItem): boolean => i.models_sold > 0 || i.gpu_configs > 0;

export function formatRoundName(r: string): string {
  const m = /^series_([a-z])$/.exec(r);
  if (m) return `Series ${m[1].toUpperCase()}`;
  return r.replace(/_/g, " ").replace(/^./, (c) => c.toUpperCase());
}
