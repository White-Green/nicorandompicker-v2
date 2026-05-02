use crate::api::AppState;
use crate::api::session::SessionId;
use axum::Extension;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::middleware::Next;
use axum::response::IntoResponse;
use http::{HeaderMap, HeaderValue, Request, Response, StatusCode, header};

const TURNSTILE_RECOMMENDED_HEADER: &str = "NRP-Turnstile-Recommended";

#[tracing::instrument(skip_all, fields(method = %request.method(), path = %request.uri().path()))]
pub(super) async fn middleware(
    State(state): State<AppState>,
    session_id: Option<Extension<SessionId>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, RateLimitError> {
    let (rate_limit_kind, actor_kind) = if let Some(Extension(session_id)) = session_id {
        let outcome = state.authenticated_rate_limiter.limit(format!("session:{}", session_id.as_str())).await?;
        if !outcome.success {
            tracing::warn!(
                actor_kind = "session",
                rate_limit_kind = ?RateLimitKind::Authenticated,
                "rate limit exceeded"
            );
            drain_body(request).await;
            return Err(RateLimitError::Exceeded(RateLimitKind::Authenticated));
        }
        (RateLimitKind::Authenticated, "session")
    } else {
        let ip = client_ip(request.headers());
        let outcome = state.anonymous_rate_limiter.limit(ip).await?;
        if !outcome.success {
            tracing::warn!(
                actor_kind = "ip",
                rate_limit_kind = ?RateLimitKind::Anonymous,
                "rate limit exceeded"
            );
            drain_body(request).await;
            return Err(RateLimitError::Exceeded(RateLimitKind::Anonymous));
        }
        (RateLimitKind::Anonymous, "ip")
    };

    let mut response = next.run(request).await;
    let status = response.status();
    tracing::info!(
        %status,
        actor_kind = actor_kind,
        rate_limit_kind = ?rate_limit_kind,
        "api request completed"
    );
    rate_limit_kind.apply_response_headers(response.headers_mut());
    Ok(response)
}

pub(super) fn client_ip(headers: &HeaderMap) -> String {
    if let Some(ip_header) = headers.get("cf-connecting-ip")
        && let Some(ip) = ip_header.to_str().ok()
        && !ip.is_empty()
    {
        ip.to_owned()
    } else {
        "unknown".to_owned()
    }
}

async fn drain_body(request: Request<Body>) {
    if let Err(err) = to_bytes(request.into_body(), 1024 * 1024).await {
        tracing::warn!(error = %err, "failed to drain rate-limited request body");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RateLimitKind {
    Authenticated,
    Anonymous,
}

impl RateLimitKind {
    fn retry_after_seconds(self) -> u64 {
        match self {
            RateLimitKind::Authenticated => 10,
            RateLimitKind::Anonymous => 60,
        }
    }

    fn apply_response_headers(self, headers: &mut header::HeaderMap) {
        if self == RateLimitKind::Anonymous {
            headers.insert(TURNSTILE_RECOMMENDED_HEADER, HeaderValue::from_static("true"));
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum RateLimitError {
    #[error("rate limit exceeded")]
    Exceeded(RateLimitKind),
    #[error("rate limiter failed: {0}")]
    Worker(#[from] worker::Error),
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> axum::response::Response {
        match self {
            RateLimitError::Exceeded(kind) => {
                let mut response = (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(header::RETRY_AFTER, kind.retry_after_seconds())],
                    axum::Json(serde_json::json!({
                        "error": "リクエスト回数が多すぎます。Turnstileの検証を行うか、しばらく待ってから再試行してください。"
                    })),
                )
                    .into_response();
                kind.apply_response_headers(response.headers_mut());
                response
            }
            RateLimitError::Worker(err) => {
                tracing::error!(error = %err, "rate limiter failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(serde_json::json!({ "error": "レートリミットの確認中にエラーが発生しました" })),
                )
                    .into_response()
            }
        }
    }
}
