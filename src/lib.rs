use crate::snapshot::HTTPClient;
use http::{HeaderMap, Request, Response};
use snapshot::SnapshotClient;
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

impl HTTPClient for reqwest::Client {
    type Error = reqwest::Error;

    #[tracing::instrument(skip(self))]
    async fn request(&self, request: Request<()>) -> Result<Response<Vec<u8>>, Self::Error> {
        let request = <reqwest::Request as TryFrom<Request<&'static [u8]>>>::try_from(request.map(|()| -> &'static [u8] { &[] }))?;
        let response = self.execute(request).await?;
        let status = response.status();
        tracing::info!("received status {}", status);
        let headers: HeaderMap = response.headers().clone();
        let bytes = response.bytes().await?;
        if let Ok(s) = str::from_utf8(&bytes) {
            tracing::info!("received {s:?}");
        } else {
            tracing::info!("received binary response {bytes:?}");
        }
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
    Ok(axum::Router::new()
        .nest("/api", api::router(SnapshotClient::new(reqwest::Client::new(), app_name)))
        .oneshot(req)
        .await?)
}
