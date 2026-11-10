import createClient, { type ClientOptions } from "openapi-fetch";
import type { components, paths } from "./schema";

export type { components, paths } from "./schema";

type Schemas = components["schemas"];
export type Category = Schemas["Category"];
export type Organization = Schemas["Organization"];
export type OrganizationDetail = Schemas["OrganizationDetail"];
export type OrgKind = Schemas["OrgKind"];
export type ProductSummary = Schemas["ProductSummary"];
export type OrganizationPage = Schemas["Page_Organization"];
export type ApiErrorBody = Schemas["ErrorBody"];

/** A typed client for the Inference Hub API, generated from its OpenAPI document. */
export function createApiClient(options: ClientOptions) {
  return createClient<paths>(options);
}

export type ApiClient = ReturnType<typeof createApiClient>;
