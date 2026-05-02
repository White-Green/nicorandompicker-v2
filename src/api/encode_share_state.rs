use crate::share_state::{EncodeError, ShareState};
use axum::Json;
use axum::response::IntoResponse;
use http::StatusCode;

#[derive(Debug)]
pub(super) enum Error {
    Encode(EncodeError),
}

impl From<EncodeError> for Error {
    fn from(err: EncodeError) -> Self {
        Error::Encode(err)
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::Encode(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
        }
    }
}

#[worker::send]
#[tracing::instrument(skip_all, fields(actor_kind = "ip", actor_fingerprint = tracing::field::Empty))]
pub(super) async fn handle(Json(share_state): Json<ShareState>) -> Result<String, Error> {
    tracing::info!(
        tag_len = share_state.search.tag.len(),
        content_id_count = share_state.content_ids.len(),
        "encode share state request started"
    );
    let encoded = crate::share_state::encode(share_state)?;
    tracing::info!(encoded_len = encoded.len(), "encode share state request completed");
    Ok(encoded)
}
