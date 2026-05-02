use crate::api::AppState;
use axum::body::Body;
use axum::extract::State;
use axum::middleware::Next;
use axum_extra::extract::cookie::{Cookie, SameSite, SignedCookieJar};
use http::{Request, Response};
use time::{Duration, UtcDateTime};

const COOKIE_NAME: &str = "nrp_session";
const COOKIE_MAX_AGE_SECONDS: Duration = Duration::hours(12);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionId(String);

impl SessionId {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

pub(super) fn add_session_cookie(jar: SignedCookieJar) -> SignedCookieJar {
    jar.add(
        Cookie::build((
            COOKIE_NAME,
            create_session_cookie_value(generate_session_id(), UtcDateTime::now() + COOKIE_MAX_AGE_SECONDS),
        ))
        .max_age(COOKIE_MAX_AGE_SECONDS)
        .path("/api")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict),
    )
}

pub(super) async fn middleware(State(state): State<AppState>, mut request: Request<Body>, next: Next) -> Response<Body> {
    let cookie_jar = SignedCookieJar::from_headers(request.headers(), state.session_cookie_key.clone());
    if let Some(cookie) = cookie_jar.get(COOKIE_NAME)
        && let Some(session_id) = parse_session_cookie_at(cookie.value(), UtcDateTime::now())
    {
        request.extensions_mut().insert(session_id);
    }

    next.run(request).await
}

fn create_session_cookie_value(session_id: String, expires_at: UtcDateTime) -> String {
    format!("{session_id}:{}", expires_at.unix_timestamp())
}

fn parse_session_cookie_at(value: &str, now: UtcDateTime) -> Option<SessionId> {
    let (id, expires_at) = value.trim().split_once(':')?;
    let expires_at = expires_at.parse::<i64>().ok().and_then(|ts| UtcDateTime::from_unix_timestamp(ts).ok())?;
    if id.is_empty() || expires_at < now {
        return None;
    }

    Some(SessionId(id.to_owned()))
}

fn generate_session_id() -> String {
    (0..4).map(|_| format!("{:016x}", rand::random::<u64>())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_session_cookie_at_accepts_unexpired_cookie_value_as_opaque_identifier() {
        assert_eq!(
            parse_session_cookie_at("session-id:100", UtcDateTime::from_unix_timestamp(100).unwrap()),
            Some(SessionId("session-id".to_owned()))
        );
    }

    #[test]
    fn parse_session_cookie_at_rejects_expired_cookie_value() {
        assert!(parse_session_cookie_at("session-id:100", UtcDateTime::from_unix_timestamp(101).unwrap()).is_none());
    }

    #[test]
    fn parse_session_cookie_at_rejects_invalid_cookie_value() {
        assert!(parse_session_cookie_at("session-id", UtcDateTime::from_unix_timestamp(100).unwrap()).is_none());
        assert!(parse_session_cookie_at(":100", UtcDateTime::from_unix_timestamp(100).unwrap()).is_none());
        assert!(parse_session_cookie_at("session-id:not-number", UtcDateTime::from_unix_timestamp(100).unwrap()).is_none());
    }
}
