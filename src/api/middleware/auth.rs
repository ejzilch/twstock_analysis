use crate::api::models::ErrorResponse;
use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

/// X-API-KEY 認證中介軟體
///
/// 從 request header 取得 X-API-KEY，與環境變數 API_KEY 比對。
/// 缺少或不符時回傳 401 UNAUTHORIZED，不繼續處理請求。
/// /health 與 /health/integrity 端點不需要認證（在 router 層排除）。
pub async fn auth_middleware(headers: HeaderMap, request: Request, next: Next) -> Response {
    let expected_key = match std::env::var("API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            tracing::error!("API_KEY environment variable is not set or empty");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "INTERNAL_ERROR",
                    "Server configuration error",
                )),
            )
                .into_response();
        }
    };

    let provided_key = headers.get("X-API-KEY").and_then(|v| v.to_str().ok());

    match provided_key {
        Some(key) if key == expected_key => next.run(request).await,
        Some(_) => {
            tracing::warn!("Invalid X-API-KEY provided");
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new(
                    "UNAUTHORIZED",
                    "Missing or invalid X-API-KEY header.",
                )),
            )
                .into_response()
        }
        None => {
            tracing::warn!("Missing X-API-KEY header");
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new(
                    "UNAUTHORIZED",
                    "Missing or invalid X-API-KEY header.",
                )),
            )
                .into_response()
        }
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn dummy_handler() -> &'static str {
        "ok"
    }

    fn make_app() -> Router {
        std::env::set_var("API_KEY", "test-secret-key");
        Router::new()
            .route("/protected", get(dummy_handler))
            .layer(middleware::from_fn(auth_middleware))
    }

    #[tokio::test]
    async fn test_valid_api_key_passes() {
        let app = make_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header("X-API-KEY", "test-secret-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_missing_api_key_returns_401() {
        let app = make_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_invalid_api_key_returns_401() {
        let app = make_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header("X-API-KEY", "wrong-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
