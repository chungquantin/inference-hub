import { api } from "@/lib/api";
import type { ModelsPayload } from "@/components/directory/types";

/**
 * Every model the market lists, in a compact form for the directory's table.
 *
 * The full list is over 20,000 models, so the page does not embed it: the
 * browser asks for it when the Models tab opens. Models come as arrays with
 * the cheapest sellers listed once, which is a fifth of the size of the API's
 * rows. The result is kept for five minutes, as the market refreshes hourly.
 */
export const dynamic = "force-dynamic";

const PAGE = 500; // the largest page the API serves
const PARALLEL = 6;
const TTL_MS = 5 * 60 * 1000;

let cached: { at: number; body: string } | null = null;
let inflight: Promise<string> | null = null;

async function build(): Promise<string> {
  const page = (offset: number) =>
    api.GET("/v1/market/models", { params: { query: { min_providers: 1, sort: "providers", limit: PAGE, offset } } });

  const first = await page(0);
  if (first.error || !first.data) throw new Error("market models unavailable");
  const total = first.data.total;
  const pages = [first.data.items];
  const offsets: number[] = [];
  for (let o = PAGE; o < total; o += PAGE) offsets.push(o);
  for (let i = 0; i < offsets.length; i += PARALLEL) {
    const batch = await Promise.all(offsets.slice(i, i + PARALLEL).map(page));
    for (const r of batch) {
      if (r.error || !r.data) throw new Error("market models unavailable");
      pages.push(r.data.items);
    }
  }

  const orgIndex = new Map<string, number>();
  const orgs: ModelsPayload["orgs"] = [];
  const rows: ModelsPayload["rows"] = [];
  for (const m of pages.flat()) {
    let at = orgIndex.get(m.cheapest.slug);
    if (at === undefined) {
      at = orgs.length;
      orgIndex.set(m.cheapest.slug, at);
      orgs.push([m.cheapest.slug, m.cheapest.name, m.cheapest.logo_url ?? null]);
    }
    rows.push([
      m.model_key,
      m.maker ?? null,
      m.providers,
      m.min_blended_per_mtok,
      m.median_blended_per_mtok,
      m.min_input_per_mtok,
      m.min_output_per_mtok,
      m.max_context_window ?? null,
      m.capabilities.filter((c) => !c.startsWith("in:") && !c.startsWith("out:")).join(","),
      at,
    ]);
  }
  const payload: ModelsPayload = { orgs, rows, total };
  return JSON.stringify(payload);
}

export async function GET() {
  const now = Date.now();
  if (!cached || now - cached.at > TTL_MS) {
    // Concurrent requests share one build.
    inflight ??= build().finally(() => (inflight = null));
    try {
      cached = { at: now, body: await inflight };
    } catch {
      if (!cached) return Response.json({ error: "market models unavailable" }, { status: 502 });
      // Serve the last good copy rather than fail.
    }
  }
  return new Response(cached.body, {
    headers: { "content-type": "application/json", "cache-control": "public, max-age=300, stale-while-revalidate=600" },
  });
}
