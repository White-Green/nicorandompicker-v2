use crate::snapshot::{self, SnapshotClient};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};

mod decode_share_state;
mod encode_share_state;
mod restore_video_details;
mod search_video;

#[derive(Clone)]
pub(super) struct AppState {
    pub(super) snapshot: SnapshotClient<reqwest::Client>,
}

pub fn router(snapshot: SnapshotClient<reqwest::Client>) -> Router {
    Router::new()
        .route("/search", post(search_video::handle))
        .route("/restore_video_details", post(restore_video_details::handle))
        .route("/encode_share_state", post(encode_share_state::handle))
        .route("/decode_share_state", post(decode_share_state::handle))
        .with_state(AppState { snapshot })
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
