/**
 * Capability icons: what a model takes in, what it returns, and what it
 * supports. Drawn on a 12px pixel grid like the rest of the design, each with
 * a text label for screen readers and a tooltip.
 */

const LABELS: Record<string, string> = {
  text: "Text",
  image: "Image",
  audio: "Audio",
  video: "Video",
  file: "Files (PDF, documents)",
  tools: "Tool calling",
  json: "JSON / structured output",
  reasoning: "Reasoning",
  web: "Web search",
  caching: "Prompt caching",
};

/** Every capability, for the legend and the filter. */
export const CAPABILITIES = [
  { id: "in:image", group: "Input", label: "Image input" },
  { id: "in:audio", group: "Input", label: "Audio input" },
  { id: "in:video", group: "Input", label: "Video input" },
  { id: "in:file", group: "Input", label: "File input" },
  { id: "out:image", group: "Output", label: "Image output" },
  { id: "out:audio", group: "Output", label: "Audio output" },
  { id: "tools", group: "Feature", label: "Tool calling" },
  { id: "json", group: "Feature", label: "Structured output" },
  { id: "reasoning", group: "Feature", label: "Reasoning" },
  { id: "web", group: "Feature", label: "Web search" },
  { id: "caching", group: "Feature", label: "Prompt caching" },
] as const;

/** Display order: modalities by how common, then features by how useful. */
const ORDER = [
  "in:image", "in:file", "in:audio", "in:video",
  "out:image", "out:audio", "out:video", "out:file",
  "tools", "json", "reasoning", "web", "caching",
];

/** 12×12 pixel glyphs, one path each. */
const GLYPHS: Record<string, string> = {
  // "T"
  text: "M2 2h8v2H7v6H5V4H2z",
  // a framed picture with a mountain
  image: "M1 2h10v8H1zM2 3v6h8V3zM3 8l2-3 2 2 1-1 1 2z",
  // a speaker with a wave
  audio: "M1 4h2l3-3v10L3 8H1zM8 3h1v6H8zM10 2h1v8h-1z",
  // a film frame with a play mark
  video: "M1 2h10v8H1zM2 3v6h8V3zM5 4l3 2-3 2z",
  // a page with a folded corner
  file: "M2 1h6l2 2v8H2zM3 2v8h6V4H7V2z",
  // a wrench
  tools: "M7 1h3v1h1v3H8L4 9v2H2v-1H1V8h2l4-4z",
  // braces
  json: "M3 1h2v1H4v3L3 6l1 1v3h1v1H3V7L2 6l1-1zM7 1h2v4l1 1-1 1v4H7v-1h1V7l1-1-1-1V2H7z",
  // a lightbulb / spark
  reasoning: "M4 1h4v1h1v4H8v2H4V6H3V2h1zM4 9h4v1H4zM5 11h2v-1H5z",
  // a globe
  web: "M4 1h4v1h1v1h1v6H9v1H8v1H4v-1H3V9H2V3h1V2h1zM5 2v8h2V2zM3 5v2h6V5z",
  // a stack / cache
  caching: "M2 2h8v2H2zM2 5h8v2H2zM2 8h8v2H2z",
};

function Glyph({ name, className }: { name: string; className?: string }) {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden shapeRendering="crispEdges" className={className}>
      <path d={GLYPHS[name] ?? GLYPHS.text} fill="currentColor" fillRule="evenodd" />
    </svg>
  );
}

function describe(cap: string): string {
  const [dir, m] = cap.includes(":") ? cap.split(":") : ["", cap];
  const label = LABELS[m] ?? m;
  if (dir === "in") return `${label} input`;
  if (dir === "out") return `${label} output`;
  return label;
}

/**
 * A model's capabilities in three tight groups: inputs (→ arrow), outputs,
 * then features. Text in and text out are the norm for chat models and are
 * not drawn, so the icons show what is special.
 */
export function Capabilities({ caps, max }: { caps: string[]; max?: number }) {
  const order = (list: string[]) => list.sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b));
  const inputs = order(caps.filter((c) => c.startsWith("in:") && c !== "in:text"));
  const outputs = order(caps.filter((c) => c.startsWith("out:") && c !== "out:text"));
  const features = order(caps.filter((c) => !c.includes(":")));
  if (inputs.length + outputs.length + features.length === 0) {
    return <span className="text-ink-3">—</span>;
  }
  const all = [...inputs, ...outputs, ...features];
  const shown = max ? all.slice(0, max) : all;
  const summary = all.map(describe).join(", ");
  const group = (list: string[], tone: string) =>
    list
      .filter((c) => shown.includes(c))
      .map((c) => (
        <span key={c} title={describe(c)} className={`inline-flex size-5 items-center justify-center ${tone}`}>
          <Glyph name={c.includes(":") ? c.split(":")[1] : c} />
        </span>
      ));
  return (
    <span className="inline-flex items-center gap-1" role="img" aria-label={summary}>
      {inputs.length > 0 && <span className="flex gap-px">{group(inputs, "bg-s-1 text-s-8")}</span>}
      {outputs.length > 0 && (
        <span className="flex items-center gap-px">
          <span aria-hidden className="px-0.5 font-mono text-[10px] text-ink-3">
            →
          </span>
          {group(outputs, "bg-warn-tint text-warn")}
        </span>
      )}
      {features.length > 0 && <span className="ml-0.5 flex gap-px">{group(features, "bg-field text-ink-2")}</span>}
      {max && all.length > max && <span className="font-mono text-[11px] text-ink-3">+{all.length - max}</span>}
    </span>
  );
}

/** A key to the icons, for the top of a table. */
export function CapabilityLegend() {
  return (
    <details className="group font-mono text-[12px] text-ink-3">
      <summary className="cursor-pointer list-none hover:text-ink">
        <span className="underline decoration-dotted">What do the icons mean?</span>
      </summary>
      <div className="mt-2 flex flex-wrap gap-x-5 gap-y-2 border border-line bg-paper p-3">
        {CAPABILITIES.map((c) => (
          <span key={c.id} className="inline-flex items-center gap-1.5">
            <span
              className={`inline-flex size-5 items-center justify-center ${
                c.group === "Input" ? "bg-s-1 text-s-8" : c.group === "Output" ? "bg-warn-tint text-warn" : "bg-field text-ink-2"
              }`}
            >
              <Glyph name={c.id.includes(":") ? c.id.split(":")[1] : c.id} />
            </span>
            {c.label}
          </span>
        ))}
        <span className="basis-full text-[11px]">
          Text in and out is not drawn. A capability shows when at least half of the providers that report it agree.
        </span>
      </div>
    </details>
  );
}
