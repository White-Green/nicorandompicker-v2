use super::{ApiResult, AppState};
use crate::snapshot::response::VideoSearchResult;
use axum::Json;
use axum::extract::State;
use std::collections::HashMap;

#[worker::send]
#[tracing::instrument(skip(state))]
pub(super) async fn handle(State(mut state): State<AppState>, Json(ids): Json<Vec<String>>) -> ApiResult<HashMap<String, VideoSearchResult>> {
    let ids = ids.into_iter().take(100).collect::<Vec<_>>();
    let details = state.snapshot.get_details(&ids).await?;
    Ok(Json(details))
}
