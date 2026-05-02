use super::{ApiResult, AppState};
use crate::logic;
use crate::snapshot::SearchCriteria;
use crate::snapshot::response::VideoSearchResult;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::Deserialize;
use time::OffsetDateTime;

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub(super) struct SearchParams {
    tag_name: String,
    video_count: usize,
    view_count_min: Option<usize>,
    view_count_max: Option<usize>,
    #[serde(default, deserialize_with = "time::serde::rfc3339::option::deserialize")]
    start_time_from: Option<OffsetDateTime>,
    #[serde(default, deserialize_with = "time::serde::rfc3339::option::deserialize")]
    start_time_to: Option<OffsetDateTime>,
}

#[worker::send]
#[tracing::instrument(skip(state))]
pub(super) async fn handle(State(state): State<AppState>, Json(params): Json<SearchParams>) -> ApiResult<Vec<VideoSearchResult>> {
    let video_count = params.video_count;
    let criteria = SearchCriteria::try_from(params)?;
    let rng = StdRng::seed_from_u64((js_sys::Math::random() * u64::MAX as f64) as u64);
    let videos = logic::collect_video_ids(state.snapshot, criteria, video_count, rng).await?;
    Ok(Json(videos))
}

#[derive(Debug, thiserror::Error)]
pub(super) enum InvalidSearchParamError {
    #[error("タグ名を入力してください")]
    TagName,
    #[error("検索件数は1件以上100件以下で指定してください")]
    VideoCount,
    #[error("再生数フィルタは下限が上限以下になるように指定してください")]
    ViewCountRange,
    #[error("アップロード日時フィルタは開始日時が終了日時以前になるように指定してください")]
    StartTimeRange,
}

impl IntoResponse for InvalidSearchParamError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": self.to_string() }))).into_response()
    }
}

impl TryFrom<SearchParams> for SearchCriteria {
    type Error = InvalidSearchParamError;

    fn try_from(value: SearchParams) -> std::result::Result<Self, Self::Error> {
        let SearchParams {
            mut tag_name,
            video_count,
            view_count_min,
            view_count_max,
            start_time_from,
            start_time_to,
        } = value;
        trim_in_place(&mut tag_name);
        if tag_name.is_empty() {
            return Err(InvalidSearchParamError::TagName);
        }
        if tag_name.len() > 1000 {
            return Err(InvalidSearchParamError::TagName);
        }

        if !(1..=100).contains(&video_count) {
            return Err(InvalidSearchParamError::VideoCount);
        }
        if view_count_min.zip(view_count_max).is_some_and(|(min, max)| min > max) {
            return Err(InvalidSearchParamError::ViewCountRange);
        }
        if start_time_from.zip(start_time_to).is_some_and(|(from, to)| from > to) {
            return Err(InvalidSearchParamError::StartTimeRange);
        }

        Ok(SearchCriteria {
            tag_name: tag_name.to_owned(),
            view_count_min,
            view_count_max,
            start_time_from,
            start_time_to,
        })
    }
}

fn trim_in_place(s: &mut String) {
    let mut iter = s.char_indices().peekable();
    while iter.next_if(|(_, c)| c.is_whitespace()).is_some() {}
    let trim_first = iter.next().map_or(s.len(), |(i, _)| i);
    s.drain(..trim_first);
    s.truncate(s.trim_end().len());
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::format_description::well_known::Rfc3339;
    use time::macros::datetime;

    #[test]
    fn search_params_deserializes_frontend_payload() {
        let payload = serde_json::json!({
            "tag_name": "VOCALOID",
            "video_count": 25,
            "view_count_min": null,
            "view_count_max": 100000,
            "start_time_from": datetime!(2025-01-01 18:30 +09:00).format(&Rfc3339).unwrap(),
            "start_time_to": null
        });
        let serialized_payload = serde_json::to_string(&payload).unwrap();
        let params: SearchParams = serde_json::from_str(&serialized_payload).unwrap();

        assert_eq!(
            params,
            SearchParams {
                tag_name: "VOCALOID".to_string(),
                video_count: 25,
                view_count_min: None,
                view_count_max: Some(100000),
                start_time_from: Some(OffsetDateTime::parse("2025-01-01T18:30:00+09:00", &Rfc3339).unwrap()),
                start_time_to: None,
            }
        );
    }

    #[test]
    fn search_params_rejects_empty_datetime_string() {
        let payload = serde_json::json!({
            "tag_name": "VOCALOID",
            "video_count": 25,
            "view_count_min": null,
            "view_count_max": null,
            "start_time_from": "",
            "start_time_to": null
        });
        let result = serde_json::from_str::<SearchParams>(&payload.to_string());

        assert!(result.is_err());
    }

    #[test]
    fn search_params_convert_to_search_criteria() {
        let params = SearchParams {
            tag_name: "  VOCALOID  ".to_string(),
            video_count: 25,
            view_count_min: Some(100),
            view_count_max: Some(200),
            start_time_from: Some(datetime!(2025-01-01 18:30 +09:00)),
            start_time_to: Some(datetime!(2025-01-02 18:30 +09:00)),
        };

        let criteria = SearchCriteria::try_from(params).unwrap();

        assert_eq!(
            criteria,
            SearchCriteria {
                tag_name: "VOCALOID".to_string(),
                view_count_min: Some(100),
                view_count_max: Some(200),
                start_time_from: Some(datetime!(2025-01-01 18:30 +09:00)),
                start_time_to: Some(datetime!(2025-01-02 18:30 +09:00)),
            }
        );
    }

    #[test]
    fn search_params_rejects_reversed_view_count_range() {
        let params = SearchParams {
            tag_name: "VOCALOID".to_string(),
            video_count: 25,
            view_count_min: Some(200),
            view_count_max: Some(100),
            start_time_from: None,
            start_time_to: None,
        };

        assert!(matches!(SearchCriteria::try_from(params), Err(InvalidSearchParamError::ViewCountRange)));
    }

    #[test]
    fn search_params_rejects_reversed_start_time_range() {
        let params = SearchParams {
            tag_name: "VOCALOID".to_string(),
            video_count: 25,
            view_count_min: None,
            view_count_max: None,
            start_time_from: Some(datetime!(2025-01-02 18:30 +09:00)),
            start_time_to: Some(datetime!(2025-01-01 18:30 +09:00)),
        };

        assert!(matches!(SearchCriteria::try_from(params), Err(InvalidSearchParamError::StartTimeRange)));
    }

    #[test]
    fn search_params_allows_one_sided_ranges() {
        let params = SearchParams {
            tag_name: "VOCALOID".to_string(),
            video_count: 25,
            view_count_min: Some(100),
            view_count_max: None,
            start_time_from: None,
            start_time_to: Some(datetime!(2025-01-01 18:30 +09:00)),
        };

        assert!(SearchCriteria::try_from(params).is_ok());
    }
}
