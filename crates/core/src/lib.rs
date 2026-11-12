//! Domain types shared by the API, the worker and the database layer.
//!
//! These types are the public contract of the API: they derive `ToSchema` so
//! the OpenAPI document (and the generated TypeScript client) follows them.

pub mod category;
pub mod compare;
pub mod market;
pub mod news;
pub mod organization;
pub mod pagination;
pub mod product;
pub mod search;

pub use category::Category;
pub use compare::{
    FundingRound, GpuPrice, GpuPriceQuery, MakerRef, ModelPrice, ModelPriceQuery, ModelPriceSort,
    PortfolioCompany, PortfolioRound, PricePoint, RepoLeader, RepoStats, RoundInvestor,
};
pub use market::{
    FailedJob, GpuIndexRow, HoldResolution, KindCount, MakerCount, MarketDetail, MarketIndexQuery,
    MarketListing, MarketOverview, MarketProvider, MarketSource, MediaIndexQuery, MediaIndexRow,
    ModelIndexRow, PriceHold, ProviderOffering, ProviderOfferings, ProviderOfferingsQuery,
};
pub use news::{
    AdapterStatus, NewsEnrichment, NewsItem, NewsMatch, NewsMention, NewsStatus, SourceStatus,
    UnmappedListing,
};
pub use organization::{OrgKind, OrgStatus, Organization, OrganizationDetail, OrganizationFeed};
pub use pagination::{Page, PageParams};
pub use product::{
    ModelSpec, Offering, OrganizationRef, ProductDetail, ProductSummary, ProductType,
};
pub use search::{SearchCount, SearchHit, SearchQuery, SearchResults};
