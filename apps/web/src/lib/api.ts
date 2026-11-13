import "server-only";

import { createApiClient } from "@inference-hub/api-client";

/**
 * Server-side API client. Pages are rendered on the server, so the browser
 * never talks to the Rust API directly yet.
 *
 * Pages render on request (the root layout calls `connection()`), so the build
 * needs no API. Responses are cached for five minutes by Next's data cache.
 */
export const api = createApiClient({
  baseUrl: process.env.API_URL ?? "http://127.0.0.1:4000",
  fetch: (request) => fetch(request, { next: { revalidate: 300 } }),
});
