/// API 中介軟體模組
///
/// auth:       X-API-KEY 認證，所有需認證的路由皆套用
/// rate_limit: IP 層級限流，每 IP 每分鐘 60 次請求
/// error:      ApiError -> HTTP response 的統一轉換
pub mod auth;
pub mod error;
pub mod rate_limit;

pub use auth::auth_middleware;
pub use error::ApiError;
pub use rate_limit::{new_rate_limiter_state, rate_limit_middleware, RateLimiterState};
