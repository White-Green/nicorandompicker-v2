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
#[tracing::instrument]
pub(super) async fn handle(Json(share_state): Json<ShareState>) -> Result<String, Error> {
    let encoded = crate::share_state::encode(share_state)?;
    Ok(encoded)
}
