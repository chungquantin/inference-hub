/** Liveness for the platform's health check: answers without calling the API, so a slow backend does not restart the web service. */
export const dynamic = "force-dynamic";

export function GET() {
  return Response.json({ status: "ok" });
}
