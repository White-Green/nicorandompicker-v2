use super::{ApiResult, AppState};
use crate::snapshot::response::VideoSearchResult;
use axum::Json;
use axum::extract::State;
use std::collections::HashMap;

#[worker::send]
#[tracing::instrument(skip_all, fields(requested_count = ids.len()))]
pub(super) async fn handle(State(mut state): State<AppState>, Json(mut ids): Json<Vec<String>>) -> ApiResult<HashMap<String, VideoSearchResult>> {
    ids.truncate(100);
    tracing::info!(content_id_count = ids.len(), "restore video details request started");
    let details = state.snapshot.get_details(&ids).await?;
    tracing::info!(result_count = details.len(), "restore video details request completed");
    Ok(Json(details))
}
