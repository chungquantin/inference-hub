import createClient, { type ClientOptions } from "openapi-fetch";
import type { components, paths } from "./schema";

export type { components, paths } from "./schema";

type Schemas = components["schemas"];
export type Category = Schemas["Category"];
export type Organization = Schemas["Organization"];
export type OrganizationDetail = Schemas["OrganizationDetail"];
export type OrgKind = Schemas["OrgKind"];
export type ProductSummary = Schemas["ProductSummary"];
export type ProductDetail = Schemas["ProductDetail"];
export type ProductType = Schemas["ProductType"];
export type ModelSpec = Schemas["ModelSpec"];
export type Offering = Schemas["Offering"];
export type OrganizationPage = Schemas["Page_Organization"];
export type ProductPage = Schemas["Page_ProductSummary"];
export type SitemapIndex = Schemas["SitemapIndex"];
export type NewsItem = Schemas["NewsItem"];
export type NewsStatus = Schemas["NewsStatus"];
export type SourceStatus = Schemas["SourceStatus"];
export type ApiErrorBody = Schemas["ErrorBody"];

/** A typed client for the Inference Hub API, generated from its OpenAPI document. */
export function createApiClient(options: ClientOptions) {
  return createClient<paths>(options);
}

export type ApiClient = ReturnType<typeof createApiClient>;
