use crate::logic::{ContentId, SearchBackend, SearchPage, SortDirection};
use axum::Json;
use axum::response::IntoResponse;
use http::{Request, Response, StatusCode};
use request::{SnapshotRequestError, SortField, build_get_details_request, build_search_request};
use response::{ErrorResponse, OkResponse, SnapshotResponse, VideoSearchResult, parse_snapshot_response};
use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;
use thiserror::Error;
use time::OffsetDateTime;
use worker::Delay;

pub mod request;
pub mod response;

const GET_DETAILS_CHUNK_SIZE: usize = 25;

#[derive(Clone)]
pub struct SnapshotClient<C> {
    http_client: C,
    app_name: String,
    previous_snapshot_request_millis: Option<f64>,
}

impl<C> SnapshotClient<C> {
    pub fn new(http_client: C, app_name: impl Into<String>) -> Self {
        Self {
            http_client,
            app_name: app_name.into(),
            previous_snapshot_request_millis: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCriteria {
    pub tag_name: String,
    pub view_count_min: Option<usize>,
    pub view_count_max: Option<usize>,
    pub start_time_from: Option<OffsetDateTime>,
    pub start_time_to: Option<OffsetDateTime>,
}

#[derive(Debug, Error)]
pub enum SearchError<H> {
    #[error("failed to build snapshot request: {0}")]
    Request(#[from] SnapshotRequestError),
    #[error("snapshot HTTP request failed: {0}")]
    HttpClient(H),
    #[error("invalid snapshot response JSON: {0}")]
    InvalidSnapshotResponse(#[source] serde_json::Error),
    #[error("snapshot API returned {status}: {error_code}: {error_message}")]
    SnapshotApi {
        status: StatusCode,
        error_code: String,
        error_message: String,
    },
}

impl<H> IntoResponse for SearchError<H>
where
    H: Error,
{
    fn into_response(self) -> axum::response::Response {
        if self.is_internal_hidden() {
            self.log_internal_error();
        }
        let status = self.status_code();
        let message = self.public_message();
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

impl<H> SearchError<H> {
    fn from_http_client(error: H) -> Self {
        SearchError::HttpClient(error)
    }

    fn status_code(&self) -> StatusCode {
        match self {
            SearchError::SnapshotApi { status, .. } if status.is_client_error() => StatusCode::BAD_REQUEST,
            SearchError::SnapshotApi { status, .. } if *status == StatusCode::SERVICE_UNAVAILABLE => StatusCode::SERVICE_UNAVAILABLE,
            SearchError::SnapshotApi { .. } | SearchError::HttpClient(_) | SearchError::InvalidSnapshotResponse(_) | SearchError::Request(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    fn public_message(&self) -> Cow<'static, str> {
        match self {
            SearchError::SnapshotApi {
                status,
                error_code,
                error_message,
            } if status.is_client_error() => Cow::Owned(format!("検索サーバに検索条件が拒否されました: {error_code}: {error_message}")),
            SearchError::SnapshotApi {
                status,
                error_message,
                error_code,
            } if *status == StatusCode::SERVICE_UNAVAILABLE => Cow::Owned(format!("検索サーバを利用できません: {error_code}: {error_message}")),
            SearchError::SnapshotApi { .. } | SearchError::HttpClient(_) | SearchError::InvalidSnapshotResponse(_) | SearchError::Request(_) => {
                Cow::Borrowed("検索中にエラーが発生しました")
            }
        }
    }

    fn is_internal_hidden(&self) -> bool {
        match self {
            SearchError::SnapshotApi { status, .. } if status.is_client_error() => false,
            SearchError::SnapshotApi { status, .. } if *status == StatusCode::SERVICE_UNAVAILABLE => false,
            SearchError::SnapshotApi { .. } | SearchError::HttpClient(_) | SearchError::InvalidSnapshotResponse(_) | SearchError::Request(_) => true,
        }
    }

    fn log_internal_error(&self) {
        match self {
            SearchError::SnapshotApi { status, error_code, .. } => {
                tracing::error!(%status, error_code = error_code.as_str(), "snapshot API returned hidden error");
            }
            SearchError::HttpClient(_) => {
                tracing::error!("snapshot HTTP request failed");
            }
            SearchError::InvalidSnapshotResponse(err) => {
                tracing::error!(error = %err, "invalid snapshot response JSON");
            }
            SearchError::Request(err) => {
                tracing::error!(error = %err, "failed to build snapshot request");
            }
        }
    }
}

impl<H> From<ErrorResponse> for SearchError<H> {
    fn from(response: ErrorResponse) -> Self {
        let status = StatusCode::from_u16(response.meta.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        SearchError::SnapshotApi {
            status,
            error_code: response.meta.error_code,
            error_message: response.meta.error_message,
        }
    }
}

pub trait HTTPClient {
    type Error: Error;
    async fn request(&self, request: Request<()>) -> worker::Result<Response<Vec<u8>>, Self::Error>;
}

impl ContentId for VideoSearchResult {
    fn content_id(&self) -> &str {
        &self.content_id
    }
}

impl<C> SearchBackend<SearchCriteria> for SnapshotClient<C>
where
    C: HTTPClient,
{
    type Error = SearchError<C::Error>;
    type SortField = SortField;
    const SORT_SPECS: &'static [Self::SortField] = &[
        SortField::ViewCounter,
        SortField::MylistCounter,
        SortField::LengthSeconds,
        SortField::StartTime,
        SortField::CommentCounter,
        SortField::LastCommentTime,
    ];
    type Result = VideoSearchResult;

    #[tracing::instrument(skip_all)]
    async fn search(
        &mut self,
        query: &SearchCriteria,
        sort_field: &Self::SortField,
        sort_direction: SortDirection,
        limit: usize,
        offset: usize,
    ) -> Result<SearchPage<Self::Result>, Self::Error> {
        tracing::debug!(?sort_field, ?sort_direction, limit, offset, "snapshot search request started");
        let request = build_search_request(&self.app_name, query, sort_field, sort_direction, limit, offset)?;
        let response = self.request_snapshot(request).await?;
        let response = parse_ok_response(response)?;
        Ok(SearchPage {
            total_count: response.meta.total_count,
            result: response.data,
        })
    }
}

impl<C> SnapshotClient<C>
where
    C: HTTPClient,
{
    #[tracing::instrument(skip_all)]
    pub async fn get_details(&mut self, video_ids: &[String]) -> Result<HashMap<String, VideoSearchResult>, SearchError<C::Error>> {
        if video_ids.is_empty() {
            return Ok(HashMap::new());
        }

        tracing::debug!(content_id_count = video_ids.len(), "snapshot details request started");
        let mut details = HashMap::new();
        for chunk in video_ids.chunks(GET_DETAILS_CHUNK_SIZE) {
            tracing::debug!(chunk_size = chunk.len(), "snapshot details chunk request started");
            let request = build_get_details_request(&self.app_name, chunk)?;
            let response = self.request_snapshot(request).await?;
            details.extend(snapshot_details_to_map(parse_ok_response(response)?));
        }
        Ok(details)
    }

    async fn request_snapshot(&mut self, request: Request<()>) -> Result<Response<Vec<u8>>, SearchError<C::Error>> {
        if let Some(previous_request_millis) = self.previous_snapshot_request_millis {
            let wait_millis = previous_request_millis.ceil().max(0.0) as u64;
            if wait_millis > 0 {
                tracing::debug!(wait_millis, "waiting before consecutive snapshot request");
                Delay::from(Duration::from_millis(wait_millis)).await;
            }
        }

        let started_at = js_sys::Date::now();
        let response = self.http_client.request(request).await.map_err(SearchError::from_http_client);
        let elapsed_millis = (js_sys::Date::now() - started_at).max(0.0);
        self.previous_snapshot_request_millis = Some(elapsed_millis);

        response
    }
}

fn parse_ok_response<H>(response: http::Response<Vec<u8>>) -> Result<OkResponse, SearchError<H>> {
    match parse_snapshot_response(response).map_err(SearchError::InvalidSnapshotResponse)? {
        SnapshotResponse::Ok(response) => Ok(response),
        SnapshotResponse::Error(response) => Err(response.into()),
    }
}

fn snapshot_details_to_map(json: OkResponse) -> HashMap<String, VideoSearchResult> {
    json.data
        .into_iter()
        .map(|data| {
            let content_id = data.content_id.clone();
            (content_id, data)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::response::ErrorMeta;

    #[test]
    fn search_error_classifies_public_and_hidden_messages() {
        let client_error: SearchError<String> = SearchError::from(ErrorResponse {
            meta: ErrorMeta {
                status: 400,
                error_code: "QUERY_PARSE_ERROR".to_string(),
                error_message: "query parse error".to_string(),
            },
        });
        assert_eq!(client_error.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(
            client_error.public_message(),
            Cow::Borrowed("検索サーバに検索条件が拒否されました: QUERY_PARSE_ERROR: query parse error")
        );
        assert!(!client_error.is_internal_hidden());

        let upstream_error: SearchError<String> = SearchError::from(ErrorResponse {
            meta: ErrorMeta {
                status: 503,
                error_code: "MAINTENANCE".to_string(),
                error_message: "please retry later.".to_string(),
            },
        });
        assert_eq!(upstream_error.status_code(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            upstream_error.public_message(),
            Cow::Borrowed("検索サーバを利用できません: MAINTENANCE: please retry later.")
        );
        assert!(!upstream_error.is_internal_hidden());

        let hidden_error = SearchError::HttpClient("connection reset".to_string());
        assert_eq!(hidden_error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(hidden_error.public_message(), Cow::Borrowed("検索中にエラーが発生しました"));
        assert!(hidden_error.is_internal_hidden());
    }
}
