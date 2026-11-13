import Link from "next/link";
import type { ModelFacts } from "@inference-hub/api-client";
import { Frame } from "@/components/ui/primitives";
import { formatTokens } from "@/lib/format";

const month = (iso: string) => new Date(iso).toLocaleDateString("en-US", { month: "short", year: "numeric", timeZone: "UTC" });
const cutoff = (s: string) => {
  const m = /^(\d{4})-(\d{2})/.exec(s);
  return m ? month(`${m[1]}-${m[2]}-01`) : s;
};
const list = (xs: string[]) => (xs.length ? xs.join(", ") : "—");

/**
 * What is known of a model apart from its price: when it came out, what it reads and writes, how much
 * it takes in and puts out, whether the weights are open. From models.dev, which groups the model
 * across providers; limits are the largest any provider states.
 */
export function FactsPanel({ facts }: { facts: ModelFacts }) {
  const caps = [
    facts.reasoning && "Reasoning",
    facts.tool_call && "Tool calls",
    facts.structured_output && "Structured output",
    facts.attachment && "File attachments",
  ].filter(Boolean) as string[];
  const rows: [string, React.ReactNode][] = [
    ["Released", facts.released_on ? month(facts.released_on) : "—"],
    ["Knowledge cutoff", facts.knowledge_cutoff ? cutoff(facts.knowledge_cutoff) : "—"],
    ["Weights", facts.open_weights == null ? "—" : facts.open_weights ? "Open" : "Closed"],
    ["Context window", facts.context_window ? `${formatTokens(facts.context_window)} tokens` : "—"],
    ["Max output", facts.max_output ? `${formatTokens(facts.max_output)} tokens` : "—"],
    ["Reads", list(facts.input_modalities)],
    ["Writes", list(facts.output_modalities)],
    ["Family", facts.family ?? "—"],
  ];
  return (
    <Frame as="section" className="flex flex-col">
      <header className="flex flex-wrap items-baseline justify-between gap-2 border-b border-line px-4 py-3">
        <h2 className="font-display text-[19px] leading-6 font-medium tracking-[-0.03em] text-ink">About this model</h2>
        <span className="text-[13px] text-ink-3">
          From{" "}
          <Link href="/organizations/models-dev" className="underline hover:text-s-7">
            models.dev
          </Link>
          {facts.providers_listing > 1 ? `, across ${facts.providers_listing} providers` : ""}
        </span>
      </header>
      {facts.description && <p className="max-w-[72ch] px-4 pt-3 text-[15px] leading-6 text-ink-2">{facts.description}</p>}
      <dl className="grid grid-cols-2 gap-x-6 gap-y-3 px-4 py-4 sm:grid-cols-4">
        {rows.map(([k, v]) => (
          <div key={k} className="min-w-0">
            <dt className="text-[13px] text-ink-3">{k}</dt>
            <dd className="truncate text-[15px] text-ink" title={typeof v === "string" ? v : undefined}>
              {v}
            </dd>
          </div>
        ))}
      </dl>
      {(caps.length > 0 || facts.deprecated) && (
        <ul className="flex flex-wrap gap-1.5 border-t border-line px-4 py-3 text-[13px]">
          {caps.map((c) => (
            <li key={c} className="bg-field px-2 py-1 text-ink-2">
              {c}
            </li>
          ))}
          {facts.deprecated && <li className="bg-warn-tint px-2 py-1 text-warn">Deprecated by its providers</li>}
        </ul>
      )}
    </Frame>
  );
}
