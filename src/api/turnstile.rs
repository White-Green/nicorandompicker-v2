use crate::api::{AppState, rate_limit, session};
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use axum_extra::extract::cookie::SignedCookieJar;
use http::{HeaderMap, StatusCode, header};
use serde::{Deserialize, Serialize};

const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VerifyRequest {
    token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VerifyResult {
    success: bool,
}

#[derive(Debug, Serialize)]
struct SiteverifyRequest<'a> {
    secret: &'a str,
    response: &'a str,
}

#[derive(Debug, Deserialize)]
struct SiteverifyResponse {
    success: bool,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error("Turnstileの検証に失敗しました")]
    Rejected,
    #[error("rate limit exceeded")]
    RateLimitExceeded(RateLimitKind),
    #[error("rate limiter failed: {0}")]
    RateLimiter(#[from] worker::Error),
    #[error("failed to build Turnstile verification request: {0}")]
    BuildRequest(#[from] serde_urlencoded::ser::Error),
    #[error("invalid Turnstile verification response: {0}")]
    InvalidResponse(#[from] serde_json::Error),
    #[error("Turnstileの検証中にエラーが発生しました: {0}")]
    Request(#[from] reqwest::Error),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::RateLimitExceeded(kind) => (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, kind.retry_after_seconds())],
                Json(serde_json::json!({
                    "error": "Turnstileの検証回数が多すぎます。しばらく待ってから再試行してください。"
                })),
            )
                .into_response(),
            Error::RateLimiter(err) => {
                tracing::error!(error = %err, "turnstile rate limiter failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "レートリミットの確認中にエラーが発生しました" })),
                )
                    .into_response()
            }
            Error::BuildRequest(err) => {
                tracing::error!(error = %err, "failed to build turnstile verification request");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "Turnstileの検証中にエラーが発生しました" })),
                )
                    .into_response()
            }
            Error::InvalidResponse(err) => {
                tracing::error!(error = %err, "invalid turnstile verification response");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "Turnstileの検証中にエラーが発生しました" })),
                )
                    .into_response()
            }
            Error::Request(err) => {
                tracing::error!(
                    status = ?err.status(),
                    ?err,
                    "turnstile siteverify request failed"
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "Turnstileの検証中にエラーが発生しました" })),
                )
                    .into_response()
            }
            Error::Rejected => (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": "Turnstileの検証に失敗しました" })),
            )
                .into_response(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RateLimitKind {
    Burst,
    Client,
}

impl RateLimitKind {
    fn retry_after_seconds(self) -> u64 {
        match self {
            RateLimitKind::Burst => 10,
            RateLimitKind::Client => 60,
        }
    }
}

#[worker::send]
#[tracing::instrument(skip_all)]
pub(super) async fn handle(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: SignedCookieJar,
    Json(request): Json<VerifyRequest>,
) -> Result<(SignedCookieJar, Json<VerifyResult>), Error> {
    check_rate_limit(&state, &headers).await?;
    tracing::info!("turnstile verification started");

    let secret_key = state.turnstile_secret_key;

    let response = reqwest::Client::new()
        .post(SITEVERIFY_URL)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(serde_urlencoded::to_string(SiteverifyRequest {
            secret: &secret_key,
            response: &request.token,
        })?)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let response = serde_json::from_slice::<SiteverifyResponse>(&response)?;

    if !response.success {
        tracing::warn!(
            error_codes = ?response.error_codes,
            "turnstile token rejected"
        );
        return Err(Error::Rejected);
    }

    tracing::info!("turnstile verification accepted");
    Ok((session::add_session_cookie(jar), Json(VerifyResult { success: true })))
}

async fn check_rate_limit(state: &AppState, headers: &HeaderMap) -> Result<(), Error> {
    let ip = rate_limit::client_ip(headers);
    let key = format!("ip:{ip}");

    let outcome = state.turnstile_verify_client_rate_limiter.limit(key.clone()).await?;
    if !outcome.success {
        tracing::warn!(
            actor_kind = "ip",
            rate_limit_kind = ?RateLimitKind::Client,
            "turnstile rate limit exceeded"
        );
        return Err(Error::RateLimitExceeded(RateLimitKind::Client));
    }

    let outcome = state.turnstile_verify_burst_rate_limiter.limit(key).await?;
    if !outcome.success {
        tracing::warn!(
            actor_kind = "ip",
            rate_limit_kind = ?RateLimitKind::Burst,
            "turnstile rate limit exceeded"
        );
        return Err(Error::RateLimitExceeded(RateLimitKind::Burst));
    }

    tracing::info!(actor_kind = "ip", "turnstile rate limit checked");
    Ok(())
}
