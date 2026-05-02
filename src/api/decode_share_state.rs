use crate::api::AppState;
use crate::share_state::{DecodeError, SearchState};
use crate::snapshot::response::VideoSearchResult;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use http::StatusCode;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DecodedShareState {
    search: SearchState,
    contents: Vec<VideoSearchResult>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error("共有データが不正です: {0}")]
    Decode(#[from] DecodeError),
    #[error("サーバ内部でエラーが発生しました: {0}")]
    Snapshot(#[from] crate::snapshot::SearchError<reqwest::Error>),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::Decode(err) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": err.to_string() }))).into_response(),
            Error::Snapshot(err) => err.into_response(),
        }
    }
}

#[worker::send]
#[tracing::instrument(skip_all)]
pub(super) async fn handle(State(mut state): State<AppState>, shared: String) -> Result<Json<DecodedShareState>, Error> {
    tracing::info!(shared_len = shared.len(), "decode share state request started");
    let share_state = crate::share_state::decode(&shared)?;
    let ids = share_state.content_ids.iter().map(ToString::to_string).collect::<Vec<_>>();
    tracing::info!(
        tag_len = share_state.search.tag.len(),
        content_id_count = ids.len(),
        "decode share state parsed"
    );
    let details = state.snapshot.get_details(&ids).await?;
    let contents = ids.into_iter().filter_map(|id| details.get(&id).cloned()).collect::<Vec<_>>();
    tracing::info!(result_count = contents.len(), "decode share state request completed");

    Ok(Json(DecodedShareState {
        search: share_state.search,
        contents,
    }))
}
