use crate::snapshot::{self, SnapshotClient};
use axum::extract::FromRef;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router, middleware};
use axum_extra::extract::cookie::Key;
use std::sync::Arc;
use worker::{RateLimiter, Result};

mod decode_share_state;
mod encode_share_state;
mod rate_limit;
mod restore_video_details;
mod search_video;
mod session;
mod turnstile;

#[derive(Clone)]
pub(super) struct AppState {
    authenticated_rate_limiter: Arc<RateLimiter>,
    anonymous_rate_limiter: Arc<RateLimiter>,
    turnstile_verify_burst_rate_limiter: Arc<RateLimiter>,
    turnstile_verify_client_rate_limiter: Arc<RateLimiter>,
    snapshot: SnapshotClient<reqwest::Client>,
    turnstile_secret_key: String,
    session_cookie_key: Key,
}

impl AppState {
    pub(super) fn new(
        authenticated_rate_limiter: Arc<RateLimiter>,
        anonymous_rate_limiter: Arc<RateLimiter>,
        turnstile_verify_burst_rate_limiter: Arc<RateLimiter>,
        turnstile_verify_client_rate_limiter: Arc<RateLimiter>,
        snapshot: SnapshotClient<reqwest::Client>,
        turnstile_secret_key: String,
        session_cookie_key: Key,
    ) -> AppState {
        AppState {
            authenticated_rate_limiter,
            anonymous_rate_limiter,
            turnstile_verify_burst_rate_limiter,
            turnstile_verify_client_rate_limiter,
            snapshot,
            turnstile_secret_key,
            session_cookie_key,
        }
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.session_cookie_key.clone()
    }
}

pub fn router(state: AppState) -> Result<Router> {
    let protected_routes = Router::new()
        .route("/search", post(search_video::handle))
        .route("/restore_video_details", post(restore_video_details::handle))
        .route("/decode_share_state", post(decode_share_state::handle))
        .route_layer(middleware::from_fn_with_state(state.clone(), rate_limit::middleware))
        .route_layer(middleware::from_fn_with_state(state.clone(), session::middleware));

    Ok(Router::new()
        .merge(protected_routes)
        .route("/encode_share_state", post(encode_share_state::handle))
        .route("/turnstile/verify", post(turnstile::handle))
        .with_state(state))
}

#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("検索条件が不正です: {0}")]
    SearchCriteriaError(#[from] search_video::InvalidSearchParamError),
    #[error("サーバ内部でエラーが発生しました: {0}")]
    SnapshotError(#[from] snapshot::SearchError<reqwest::Error>),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            ApiError::SearchCriteriaError(err) => err.into_response(),
            ApiError::SnapshotError(err) => err.into_response(),
        }
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;
