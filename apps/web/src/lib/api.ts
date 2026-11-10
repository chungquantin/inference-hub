import "server-only";

import { createApiClient } from "@inference-hub/api-client";

/**
 * Server-side API client. Pages are rendered on the server, so the browser
 * never talks to the Rust API directly yet.
 */
export const api = createApiClient({
  baseUrl: process.env.API_URL ?? "http://127.0.0.1:4000",
});
