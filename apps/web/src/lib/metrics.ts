/** Formatting and links for the Terminal's metrics. */

const compact = (v: number, digits = 1) => {
  const abs = Math.abs(v);
  const trim = (n: number) => n.toFixed(Math.abs(n) >= 100 ? 0 : digits).replace(/\.0+$/, "");
  if (abs >= 1e12) return `${trim(v / 1e12)}T`;
  if (abs >= 1e9) return `${trim(v / 1e9)}B`;
  if (abs >= 1e6) return `${trim(v / 1e6)}M`;
  if (abs >= 1e3) return `${trim(v / 1e3)}K`;
  return trim(v);
};

/** A metric value in its unit: `$2.1B`, `1,036 tok/s`, `181 ms`, `134K`. */
export function formatMetric(unit: string, v: number | null | undefined): string {
  if (v == null) return "—";
  switch (unit) {
    case "usd":
      return `$${compact(v)}`;
    case "tokens_per_second":
      return `${Math.round(v).toLocaleString("en-US")} tok/s`;
    case "ms":
      return v >= 1000 ? `${(v / 1000).toFixed(2)} s` : `${Math.round(v)} ms`;
    default:
      return compact(v);
  }
}

/** 0.124 → "+12.4%". Large moves lose the decimal. */
export function formatChange(f: number | null | undefined): string {
  if (f == null || !Number.isFinite(f)) return "—";
  const pct = f * 100;
  const body = Math.abs(pct) >= 100 ? Math.round(Math.abs(pct)).toLocaleString("en-US") : Math.abs(pct).toFixed(1);
  return `${pct < 0 ? "−" : "+"}${body}%`;
}

/** Colour class for a change: better is green, worse is red, whichever way the metric points. */
export function changeTone(f: number | null | undefined, higherIsBetter: boolean): string {
  if (f == null || Math.abs(f) < 0.0005) return "text-ink-3";
  return f > 0 === higherIsBetter ? "text-lo" : "text-hi";
}

export const GROUP_ORDER = ["Adoption", "Serving", "Open source"];

export function metricHref(key: string): string {
  return `/analytics/metrics/${key}`;
}

/** Link for a ranked subject: its organization's page when it has one. */
export function subjectHref(row: { organization?: { slug: string } | null }, subjectType: string): string | null {
  if (subjectType === "model") return null;
  return row.organization ? `/organizations/${row.organization.slug}` : null;
}
