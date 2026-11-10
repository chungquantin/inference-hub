//! Extractors that reject with [`ApiError`], so malformed input gets the same
//! JSON error body as every other failure instead of axum's plain-text default.

use axum::extract::{FromRequest, FromRequestParts};

use crate::error::ApiError;

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(ApiError))]
pub struct Query<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub struct Path<T>(pub T);

#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Json<T>(pub T);
