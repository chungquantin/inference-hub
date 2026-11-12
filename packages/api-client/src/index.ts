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
export type ModelPrice = Schemas["ModelPrice"];
export type ModelPriceSort = Schemas["ModelPriceSort"];
export type GpuPrice = Schemas["GpuPrice"];
export type GpuComparison = Schemas["GpuComparison"];
export type PricePoint = Schemas["PricePoint"];
export type FundingRound = Schemas["FundingRound"];
export type PortfolioCompany = Schemas["PortfolioCompany"];
export type RepoStats = Schemas["RepoStats"];
export type RepoLeader = Schemas["RepoLeader"];
export type AdapterStatus = Schemas["AdapterStatus"];
export type UnmappedListing = Schemas["UnmappedListing"];
export type ApiErrorBody = Schemas["ErrorBody"];
export type SearchResults = Schemas["SearchResults"];
export type SearchHit = Schemas["SearchHit"];
export type ModelIndexRow = Schemas["ModelIndexRow"];
export type GpuIndexRow = Schemas["GpuIndexRow"];
export type MediaIndexRow = Schemas["MediaIndexRow"];
export type MakerCount = Schemas["MakerCount"];
export type MarketAnalytics = Schemas["MarketAnalytics"];
export type ModelRank = Schemas["ModelRank"];
export type GpuRank = Schemas["GpuRank"];
export type GpuCell = Schemas["GpuCell"];
export type MakerRank = Schemas["MakerRank"];
export type ProviderRank = Schemas["ProviderRank"];
export type PriceBucket = Schemas["PriceBucket"];

/** A typed client for the Inference Hub API, generated from its OpenAPI document. */
export function createApiClient(options: ClientOptions) {
  return createClient<paths>(options);
}

export type ApiClient = ReturnType<typeof createApiClient>;
