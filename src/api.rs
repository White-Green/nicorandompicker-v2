use crate::logic::SearchBackend;
use crate::snapshot::request::SortField;
use crate::snapshot::response::VideoSearchResult;
use crate::snapshot::{self, SearchCriteria, SnapshotClient};
use axum::extract::FromRef;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router, middleware};
use axum_extra::extract::cookie::Key;
use http::header::CONTENT_TYPE;
use http::{HeaderValue, Method};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use worker::{RateLimiter, Result};

const LEGACY_APP_ORIGIN: &str = "https://white-green.github.io";

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

pub(super) trait SnapshotBackend:
    Clone + SearchBackend<SearchCriteria, Error = snapshot::SearchError<reqwest::Error>, SortField = SortField, Result = VideoSearchResult>
{
    async fn get_details(&mut self, video_ids: &[String]) -> Result<HashMap<String, VideoSearchResult>, snapshot::SearchError<reqwest::Error>>;
}

impl SnapshotBackend for SnapshotClient<reqwest::Client> {
    async fn get_details(&mut self, video_ids: &[String]) -> Result<HashMap<String, VideoSearchResult>, snapshot::SearchError<reqwest::Error>> {
        SnapshotClient::get_details(self, video_ids).await
    }
}

pub(super) trait ApiState: Clone + Send + Sync + 'static {
    type Snapshot: SnapshotBackend;

    fn snapshot(&self) -> Self::Snapshot;
    fn turnstile_secret_key(&self) -> &str;
    fn session_cookie_key(&self) -> Key;
    fn authenticated_rate_limit(&self, key: String) -> impl Future<Output = worker::Result<bool>> + Send;
    fn anonymous_rate_limit(&self, key: String) -> impl Future<Output = worker::Result<bool>> + Send;
    fn turnstile_verify_burst_rate_limit(&self, key: String) -> impl Future<Output = worker::Result<bool>> + Send;
    fn turnstile_verify_client_rate_limit(&self, key: String) -> impl Future<Output = worker::Result<bool>> + Send;
}

impl ApiState for AppState {
    type Snapshot = SnapshotClient<reqwest::Client>;

    fn snapshot(&self) -> Self::Snapshot {
        self.snapshot.clone()
    }

    fn turnstile_secret_key(&self) -> &str {
        &self.turnstile_secret_key
    }

    fn session_cookie_key(&self) -> Key {
        self.session_cookie_key.clone()
    }

    async fn authenticated_rate_limit(&self, key: String) -> worker::Result<bool> {
        Ok(self.authenticated_rate_limiter.limit(key).await?.success)
    }

    async fn anonymous_rate_limit(&self, key: String) -> worker::Result<bool> {
        Ok(self.anonymous_rate_limiter.limit(key).await?.success)
    }

    async fn turnstile_verify_burst_rate_limit(&self, key: String) -> worker::Result<bool> {
        Ok(self.turnstile_verify_burst_rate_limiter.limit(key).await?.success)
    }

    async fn turnstile_verify_client_rate_limit(&self, key: String) -> worker::Result<bool> {
        Ok(self.turnstile_verify_client_rate_limiter.limit(key).await?.success)
    }
}

#[derive(Clone)]
pub(super) struct SessionCookieKey(Key);

impl<S: ApiState> FromRef<S> for SessionCookieKey {
    fn from_ref(state: &S) -> Self {
        SessionCookieKey(state.session_cookie_key())
    }
}

impl From<SessionCookieKey> for Key {
    fn from(key: SessionCookieKey) -> Self {
        key.0
    }
}

pub fn router<S: ApiState>(state: S) -> Result<Router> {
    let protected_routes = Router::new()
        .route("/search", post(search_video::handle::<S>))
        .route("/restore_video_details", post(restore_video_details::handle::<S>))
        .route("/decode_share_state", post(decode_share_state::handle::<S>))
        .route_layer(middleware::from_fn_with_state(state.clone(), rate_limit::middleware::<S>))
        .route_layer(middleware::from_fn_with_state(state.clone(), session::middleware::<S>));

    Ok(Router::new()
        .merge(protected_routes)
        .route(
            "/encode_share_state",
            post(encode_share_state::handle).layer(
                CorsLayer::new()
                    .allow_origin(HeaderValue::from_static(LEGACY_APP_ORIGIN))
                    .allow_methods([Method::POST])
                    .allow_headers([CONTENT_TYPE]),
            ),
        )
        .route("/turnstile/verify", post(turnstile::handle::<S>))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::{SearchPage, SortDirection};
    use axum::body::Body;
    use http::header::{ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN};
    use http::{Request, StatusCode};
    use tower::ServiceExt;

    #[derive(Clone)]
    struct TestSnapshot;

    impl SearchBackend<SearchCriteria> for TestSnapshot {
        type Error = snapshot::SearchError<reqwest::Error>;
        type SortField = SortField;
        const SORT_SPECS: &'static [Self::SortField] = &[SortField::ViewCounter];
        type Result = VideoSearchResult;

        async fn search(
            &mut self,
            _query: &SearchCriteria,
            _sort_field: &Self::SortField,
            _sort_direction: SortDirection,
            _limit: usize,
            _offset: usize,
        ) -> Result<SearchPage<Self::Result>, Self::Error> {
            unreachable!("snapshot search is not used by CORS preflight requests")
        }
    }

    impl SnapshotBackend for TestSnapshot {
        async fn get_details(&mut self, _video_ids: &[String]) -> Result<HashMap<String, VideoSearchResult>, snapshot::SearchError<reqwest::Error>> {
            unreachable!("snapshot details are not used by CORS preflight requests")
        }
    }

    #[derive(Clone)]
    struct TestState {
        session_cookie_key: Key,
    }

    impl TestState {
        fn new() -> Self {
            Self {
                session_cookie_key: Key::derive_from(b"test session cookie key with enough bytes"),
            }
        }
    }

    impl ApiState for TestState {
        type Snapshot = TestSnapshot;

        fn snapshot(&self) -> Self::Snapshot {
            TestSnapshot
        }

        fn turnstile_secret_key(&self) -> &str {
            "test turnstile secret"
        }

        fn session_cookie_key(&self) -> Key {
            self.session_cookie_key.clone()
        }

        async fn authenticated_rate_limit(&self, _key: String) -> worker::Result<bool> {
            Ok(true)
        }

        async fn anonymous_rate_limit(&self, _key: String) -> worker::Result<bool> {
            Ok(true)
        }

        async fn turnstile_verify_burst_rate_limit(&self, _key: String) -> worker::Result<bool> {
            Ok(true)
        }

        async fn turnstile_verify_client_rate_limit(&self, _key: String) -> worker::Result<bool> {
            Ok(true)
        }
    }

    fn preflight_request(path: &'static str) -> Request<Body> {
        Request::builder()
            .method(Method::OPTIONS)
            .uri(path)
            .header("origin", LEGACY_APP_ORIGIN)
            .header("access-control-request-method", "POST")
            .header("access-control-request-headers", "content-type")
            .body(Body::empty())
            .unwrap()
    }

    #[test]
    fn share_state_encoding_allows_requests_from_legacy_app() {
        let response = pollster::block_on(router(TestState::new()).unwrap().oneshot(preflight_request("/encode_share_state"))).unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[ACCESS_CONTROL_ALLOW_ORIGIN], LEGACY_APP_ORIGIN);
        assert_eq!(response.headers()[ACCESS_CONTROL_ALLOW_METHODS], "POST");
        assert_eq!(response.headers()[ACCESS_CONTROL_ALLOW_HEADERS], "content-type");
    }

    #[test]
    fn search_route_does_not_allow_cross_origin_requests() {
        let response = pollster::block_on(router(TestState::new()).unwrap().oneshot(preflight_request("/search"))).unwrap();

        assert!(!response.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN));
    }
}
