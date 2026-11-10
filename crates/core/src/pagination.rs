use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

pub const DEFAULT_LIMIT: i64 = 24;
pub const MAX_LIMIT: i64 = 100;

#[derive(Debug, Clone, Copy, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PageParams {
    /// Items per page (1-100, default 24).
    pub limit: Option<i64>,
    /// Items to skip (default 0).
    pub offset: Option<i64>,
}

impl PageParams {
    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }

    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
}

/// One page of results plus the total count across all pages.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_defaults_and_clamps() {
        assert_eq!(PageParams::default().limit(), DEFAULT_LIMIT);
        let p = PageParams {
            limit: Some(1_000),
            offset: Some(-5),
        };
        assert_eq!(p.limit(), MAX_LIMIT);
        assert_eq!(p.offset(), 0);
        let p = PageParams {
            limit: Some(0),
            offset: None,
        };
        assert_eq!(p.limit(), 1);
    }
}
