//! Bearer-token protection for the admin routes.

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

use crate::{AppState, error::ApiError};

/// Rejects requests without `Authorization: Bearer <ADMIN_TOKEN>`.
///
/// When no admin token is configured the admin API does not exist: every
/// request gets 404, so an unconfigured deployment exposes nothing.
pub async fn require_admin(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let Some(expected) = state.admin_token.as_deref() else {
        return ApiError::NotFound("route").into_response();
    };
    let presented = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if bool::from(presented.as_bytes().ct_eq(expected.as_bytes())) {
        next.run(request).await
    } else {
        ApiError::Unauthorized.into_response()
    }
}
