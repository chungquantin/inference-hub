//! Domain types shared by the API, the worker and the database layer.
//!
//! These types are the public contract of the API: they derive `ToSchema` so
//! the OpenAPI document (and the generated TypeScript client) follows them.

pub mod category;
pub mod organization;
pub mod pagination;
pub mod product;

pub use category::Category;
pub use organization::{OrgKind, OrgStatus, Organization, OrganizationDetail};
pub use pagination::{Page, PageParams};
pub use product::{
    ModelSpec, Offering, OrganizationRef, ProductDetail, ProductSummary, ProductType,
};
