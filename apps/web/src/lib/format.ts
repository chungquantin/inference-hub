const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  minimumFractionDigits: 2,
  maximumFractionDigits: 4,
});

/** A price in its currency, or an em dash when there is none. */
export function formatPrice(value: number | null | undefined, currency = "USD"): string {
  if (value == null) return "—";
  if (currency === "USD") return usd.format(value);
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  }).format(value);
}

/** 131072 → "131K", 2000000 → "2M". */
export function formatTokens(value: number | null | undefined): string {
  if (value == null) return "—";
  if (value >= 1_000_000) return `${trim(value / 1_000_000)}M`;
  if (value >= 1_000) return `${trim(value / 1_000)}K`;
  return String(value);
}

/** Billions of parameters: 70 → "70B", 46.7 → "46.7B", 0.5 → "500M". */
export function formatParams(billions: number | null | undefined): string {
  if (billions == null) return "—";
  if (billions < 1) return `${trim(billions * 1000)}M`;
  return `${trim(billions)}B`;
}

/** "2024-07-23" → "Jul 23, 2024". Dates are calendar dates, so format them in UTC. */
export function formatDate(value: string | null | undefined): string {
  if (!value) return "—";
  return new Date(value).toLocaleDateString("en-US", { dateStyle: "medium", timeZone: "UTC" });
}

function trim(n: number): string {
  return Number.isInteger(n) ? String(n) : n.toFixed(1).replace(/\.0$/, "");
}
