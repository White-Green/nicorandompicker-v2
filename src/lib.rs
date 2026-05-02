use crate::api::AppState;
use crate::snapshot::HTTPClient;
use http::{HeaderMap, Request, Response};
use snapshot::SnapshotClient;
use std::sync::Arc;
use tower::ServiceExt;
use tracing_subscriber::fmt::format::Pretty;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_web::MakeConsoleWriter;
use worker::{Context, Env, Error, HttpRequest, Result, event};

mod api;
mod logic;
mod share_state;
mod snapshot;

const APP_NAME_BINDING: &str = "APP_NAME";
const TURNSTILE_SECRET_KEY_BINDING: &str = "TURNSTILE_SECRET_KEY";
const SESSION_COOKIE_SECRET_BINDING: &str = "SESSION_COOKIE_SECRET";
const AUTHENTICATED_API_RATE_LIMITER_BINDING: &str = "AUTHENTICATED_API_RATE_LIMITER";
const ANONYMOUS_API_RATE_LIMITER_BINDING: &str = "ANONYMOUS_API_RATE_LIMITER";
const TURNSTILE_VERIFY_BURST_RATE_LIMITER_BINDING: &str = "TURNSTILE_VERIFY_BURST_RATE_LIMITER";
const TURNSTILE_VERIFY_CLIENT_RATE_LIMITER_BINDING: &str = "TURNSTILE_VERIFY_CLIENT_RATE_LIMITER";

impl HTTPClient for reqwest::Client {
    type Error = reqwest::Error;

    #[tracing::instrument(skip_all, err)]
    async fn request(&self, request: Request<()>) -> Result<Response<Vec<u8>>, Self::Error> {
        let request = <reqwest::Request as TryFrom<Request<&'static [u8]>>>::try_from(request.map(|()| -> &'static [u8] { &[] }))?;
        let response = self.execute(request).await?;
        let status = response.status();
        let headers: HeaderMap = response.headers().clone();
        let bytes = response.bytes().await?;
        tracing::info!(%status, response_len = bytes.len(), "received snapshot response");
        let mut builder = Response::builder().status(status);
        *builder.headers_mut().unwrap() = headers;
        Ok(builder.body(bytes.to_vec()).unwrap())
    }
}

#[event(start)]
fn start() {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .without_time()
        .with_writer(MakeConsoleWriter);
    let perf_layer = tracing_web::performance_layer().with_details_from_fields(Pretty::default());
    tracing_subscriber::registry().with(fmt_layer).with(perf_layer).init();
}

#[event(fetch)]
pub async fn fetch(req: HttpRequest, env: Env, _ctx: Context) -> Result<http::Response<axum::body::Body>> {
    console_error_panic_hook::set_once();
    let app_name = env.var(APP_NAME_BINDING)?.to_string();
    if app_name.trim().is_empty() {
        return Err(Error::RustError(format!("{APP_NAME_BINDING} must not be empty")));
    }
    let turnstile_secret_key = env.var(TURNSTILE_SECRET_KEY_BINDING)?.to_string();
    if turnstile_secret_key.trim().is_empty() {
        return Err(Error::RustError(format!("{TURNSTILE_SECRET_KEY_BINDING} must not be empty")));
    }
    let session_cookie_secret = env.var(SESSION_COOKIE_SECRET_BINDING)?.to_string();
    if session_cookie_secret.trim().is_empty() {
        return Err(Error::RustError(format!("{SESSION_COOKIE_SECRET_BINDING} must not be empty")));
    }
    let authenticated_rate_limiter = env.rate_limiter(AUTHENTICATED_API_RATE_LIMITER_BINDING)?;
    let anonymous_rate_limiter = env.rate_limiter(ANONYMOUS_API_RATE_LIMITER_BINDING)?;
    let turnstile_verify_burst_rate_limiter = env.rate_limiter(TURNSTILE_VERIFY_BURST_RATE_LIMITER_BINDING)?;
    let turnstile_verify_client_rate_limiter = env.rate_limiter(TURNSTILE_VERIFY_CLIENT_RATE_LIMITER_BINDING)?;
    let state = AppState::new(
        Arc::new(authenticated_rate_limiter),
        Arc::new(anonymous_rate_limiter),
        Arc::new(turnstile_verify_burst_rate_limiter),
        Arc::new(turnstile_verify_client_rate_limiter),
        SnapshotClient::new(reqwest::Client::new(), app_name),
        turnstile_secret_key,
        axum_extra::extract::cookie::Key::derive_from(session_cookie_secret.as_bytes()),
    );
    Ok(axum::Router::new().nest("/api", api::router(state)?).oneshot(req).await?)
}
