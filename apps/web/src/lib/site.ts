/** Absolute origin of the site, for canonical URLs, the sitemap and structured data. */
export const SITE_URL = (process.env.SITE_URL ?? "http://localhost:3000").replace(/\/$/, "");

export const SITE_NAME = "Inference Hub";

/** Serializes structured data for a `<script type="application/ld+json">` tag. */
export function jsonLd(data: Record<string, unknown>): { __html: string } {
  // Escape `<` so a value can never close the script tag.
  return { __html: JSON.stringify({ "@context": "https://schema.org", ...data }).replace(/</g, "\\u003c") };
}
