import type { NextRequest } from "next/server";
import { api } from "@/lib/api";
import { modelLogoUrl } from "@/lib/catalog";

/**
 * Search for the command palette. The browser never talks to the Rust API
 * directly; this route forwards the query and caps what it accepts.
 */
export async function GET(request: NextRequest) {
  const q = (request.nextUrl.searchParams.get("q") ?? "").trim().slice(0, 100);
  if (q.length < 2) return Response.json({ q, hits: [], counts: [] });
  const { data, error } = await api.GET("/v1/search", { params: { query: { q, per_kind: 5 } } });
  if (error || !data) return Response.json({ error: "search failed" }, { status: 502 });
  // Model logos come from the inference catalog, which lives in the web app.
  const hits = data.hits.map((h) =>
    (h.kind === "model" || h.kind === "media") && !h.logo_url
      ? { ...h, logo_url: modelLogoUrl(h.href.split("/").pop() ?? h.title, h.maker) }
      : h,
  );
  return Response.json({ ...data, hits }, { headers: { "Cache-Control": "public, max-age=60" } });
}
