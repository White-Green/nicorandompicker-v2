#![allow(unused)]
use crate::logic::SortDirection;
use crate::snapshot::SearchCriteria;
use arrayvec::ArrayVec;
use http::header::USER_AGENT;
use serde::ser::{Error, SerializeStruct};
use serde::{Serialize, Serializer};
use std::borrow::Cow;
use std::collections::Bound;
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const SNAPSHOT_URL: &str = "https://snapshot.search.nicovideo.jp/api/v2/snapshot/video/contents/search";
const SEARCH_FIELDS: &str = "contentId,title,viewCounter,commentCounter,mylistCounter,likeCounter,lengthSeconds,thumbnailUrl,tags";

#[derive(Debug, Error)]
pub enum SnapshotRequestError {
    #[error("failed to serialize jsonFilter: {0}")]
    JsonFilterSerialization(#[from] serde_json::Error),
    #[error("failed to encode snapshot query: {0}")]
    QueryEncoding(#[from] serde_urlencoded::ser::Error),
    #[error("failed to build snapshot request: {0}")]
    RequestBuild(#[from] http::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SortField {
    ViewCounter,
    MylistCounter,
    LengthSeconds,
    StartTime,
    CommentCounter,
    LastCommentTime,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RefOrBox<'a, T: ?Sized> {
    Ref(&'a T),
    Owned(Box<T>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum FilterField {
    ContentId,
    ViewCounter,
    MylistCounter,
    LikeCounter,
    LengthSeconds,
    StartTime,
    CommentCounter,
    LastCommentTime,
    CategoryTags,
    Tags,
    TagsExact,
    Genre,
    GenreKeyword,
    ContentType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentType {
    Long,
    Short,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "field", content = "value")]
pub enum FilterEqual<'a> {
    ContentId(Cow<'a, str>),
    ViewCounter(u64),
    MylistCounter(u64),
    LikeCounter(u64),
    LengthSeconds(u64),
    StartTime(#[serde(serialize_with = "time::serde::rfc3339::serialize")] OffsetDateTime),
    CommentCounter(u64),
    LastCommentTime(Cow<'a, str>),
    CategoryTags(Cow<'a, str>),
    Tags(Cow<'a, str>),
    TagsExact(Cow<'a, str>),
    Genre(Cow<'a, str>),
    GenreKeyword(Cow<'a, str>),
    ContentType(ContentType),
}

#[derive(Debug)]
pub enum FilterRange<'a> {
    ContentId((Bound<Cow<'a, str>>, Bound<Cow<'a, str>>)),
    ViewCounter((Bound<u64>, Bound<u64>)),
    MylistCounter((Bound<u64>, Bound<u64>)),
    LikeCounter((Bound<u64>, Bound<u64>)),
    LengthSeconds((Bound<u64>, Bound<u64>)),
    StartTime((Bound<OffsetDateTime>, Bound<OffsetDateTime>)),
    CommentCounter((Bound<u64>, Bound<u64>)),
    LastCommentTime((Bound<Cow<'a, str>>, Bound<Cow<'a, str>>)),
    CategoryTags((Bound<Cow<'a, str>>, Bound<Cow<'a, str>>)),
    Tags((Bound<Cow<'a, str>>, Bound<Cow<'a, str>>)),
    TagsExact((Bound<Cow<'a, str>>, Bound<Cow<'a, str>>)),
    Genre((Bound<Cow<'a, str>>, Bound<Cow<'a, str>>)),
    GenreKeyword((Bound<Cow<'a, str>>, Bound<Cow<'a, str>>)),
    ContentType((Bound<ContentType>, Bound<ContentType>)),
}

impl Serialize for FilterRange<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            FilterRange::ContentId(bounds) => serialize_range(serializer, "contentId", bounds),
            FilterRange::ViewCounter(bounds) => serialize_range(serializer, "viewCounter", bounds),
            FilterRange::MylistCounter(bounds) => serialize_range(serializer, "mylistCounter", bounds),
            FilterRange::LikeCounter(bounds) => serialize_range(serializer, "likeCounter", bounds),
            FilterRange::LengthSeconds(bounds) => serialize_range(serializer, "lengthSeconds", bounds),
            FilterRange::StartTime(bounds) => serialize_datetime_range(serializer, "startTime", bounds),
            FilterRange::CommentCounter(bounds) => serialize_range(serializer, "commentCounter", bounds),
            FilterRange::LastCommentTime(bounds) => serialize_range(serializer, "lastCommentTime", bounds),
            FilterRange::CategoryTags(bounds) => serialize_range(serializer, "categoryTags", bounds),
            FilterRange::Tags(bounds) => serialize_range(serializer, "tags", bounds),
            FilterRange::TagsExact(bounds) => serialize_range(serializer, "tagsExact", bounds),
            FilterRange::Genre(bounds) => serialize_range(serializer, "genre", bounds),
            FilterRange::GenreKeyword(bounds) => serialize_range(serializer, "genre.keyword", bounds),
            FilterRange::ContentType(bounds) => serialize_range(serializer, "contentType", bounds),
        }
    }
}

fn serialize_range<S, T>(serializer: S, field: &str, (lower, upper): &(Bound<T>, Bound<T>)) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    let from = match lower {
        Bound::Included(value) => Some((value, true)),
        Bound::Excluded(value) => Some((value, false)),
        Bound::Unbounded => None,
    };
    let to = match upper {
        Bound::Included(value) => Some((value, true)),
        Bound::Excluded(value) => Some((value, false)),
        Bound::Unbounded => None,
    };
    if from.is_none() && to.is_none() {
        return Err(S::Error::custom("range must have at least one bound"));
    }
    let mut state = serializer.serialize_struct("FilterRange", 1 + from.is_some() as usize * 2 + to.is_some() as usize * 2)?;
    state.serialize_field("field", field)?;
    if let Some((from, include_lower)) = from {
        state.serialize_field("from", from)?;
        state.serialize_field("include_lower", &include_lower)?;
    }
    if let Some((to, include_upper)) = to {
        state.serialize_field("to", to)?;
        state.serialize_field("include_upper", &include_upper)?;
    }
    state.end()
}

fn serialize_datetime_range<S>(serializer: S, field: &str, (lower, upper): &(Bound<OffsetDateTime>, Bound<OffsetDateTime>)) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let from = match lower {
        Bound::Included(value) => Some((value, true)),
        Bound::Excluded(value) => Some((value, false)),
        Bound::Unbounded => None,
    };
    let to = match upper {
        Bound::Included(value) => Some((value, true)),
        Bound::Excluded(value) => Some((value, false)),
        Bound::Unbounded => None,
    };
    if from.is_none() && to.is_none() {
        return Err(S::Error::custom("range must have at least one bound"));
    }
    let mut state = serializer.serialize_struct("FilterRange", 1 + from.is_some() as usize * 2 + to.is_some() as usize * 2)?;
    state.serialize_field("field", field)?;
    if let Some((from, include_lower)) = from {
        state.serialize_field("from", &from.format(&Rfc3339).map_err(serde::ser::Error::custom)?)?;
        state.serialize_field("include_lower", &include_lower)?;
    }
    if let Some((to, include_upper)) = to {
        state.serialize_field("to", &to.format(&Rfc3339).map_err(serde::ser::Error::custom)?)?;
        state.serialize_field("include_upper", &include_upper)?;
    }
    state.end()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum JsonFilter<'a> {
    Equal(FilterEqual<'a>),
    Range(FilterRange<'a>),
    Or { filters: RefOrBox<'a, [JsonFilter<'a>]> },
    And { filters: RefOrBox<'a, [JsonFilter<'a>]> },
    Not { filter: RefOrBox<'a, JsonFilter<'a>> },
}

#[derive(Debug)]
pub enum SnapshotSearchQueryTarget<'a> {
    None,
    Query { q: &'a str, targets: &'a str },
}

impl Serialize for SnapshotSearchQueryTarget<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            SnapshotSearchQueryTarget::None => {
                let mut state = serializer.serialize_struct("SnapshotSearchQueryTarget", 1)?;
                state.serialize_field("q", "")?;
                state.end()
            }
            SnapshotSearchQueryTarget::Query { q, targets } => {
                let mut state = serializer.serialize_struct("SnapshotSearchQueryTarget", 2)?;
                state.serialize_field("q", q)?;
                state.serialize_field("targets", targets)?;
                state.end()
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SnapshotSearchQuery<'a> {
    #[serde(flatten)]
    pub q: SnapshotSearchQueryTarget<'a>,
    pub fields: &'a str,
    #[serde(rename = "jsonFilter", skip_serializing_if = "Option::is_none")]
    pub json_filter: Option<String>,
    #[serde(rename = "_sort")]
    pub sort: &'a str,
    #[serde(rename = "_limit")]
    pub limit: usize,
    #[serde(rename = "_offset")]
    pub offset: usize,
    #[serde(rename = "_context")]
    pub context: &'a str,
}

pub fn build_search_request(
    app_name: &str,
    criteria: &SearchCriteria,
    sort_field: &SortField,
    sort_direction: SortDirection,
    limit: usize,
    offset: usize,
) -> Result<http::Request<()>, SnapshotRequestError> {
    let &SearchCriteria {
        ref tag_name,
        view_count_min,
        view_count_max,
        start_time_from,
        start_time_to,
    } = criteria;

    let mut filters = ArrayVec::<_, 3>::new();
    if view_count_min.is_some() || view_count_max.is_some() {
        filters.push(JsonFilter::Range(FilterRange::ViewCounter((
            view_count_min.map_or(Bound::Unbounded, |view_count_min| Bound::Included(view_count_min as u64)),
            view_count_max.map_or(Bound::Unbounded, |view_count_max| Bound::Included(view_count_max as u64)),
        ))));
    }
    if start_time_from.is_some() || start_time_to.is_some() {
        filters.push(JsonFilter::Range(FilterRange::StartTime((
            start_time_from.map_or(Bound::Unbounded, Bound::Included),
            start_time_to.map_or(Bound::Unbounded, Bound::Included),
        ))));
    }
    let filter = (!filters.is_empty())
        .then(|| {
            serde_json::to_string(&JsonFilter::And {
                filters: RefOrBox::Ref(&filters),
            })
        })
        .transpose()
        .map_err(SnapshotRequestError::JsonFilterSerialization)?;
    let sort = sort_value(sort_field, sort_direction);
    let query = SnapshotSearchQuery {
        q: SnapshotSearchQueryTarget::Query {
            q: tag_name,
            targets: "tagsExact",
        },
        fields: SEARCH_FIELDS,
        json_filter: filter,
        sort: &sort,
        limit,
        offset,
        context: app_name,
    };
    build_request(app_name, query)
}

pub fn build_get_details_request(app_name: &str, video_ids: &[String]) -> Result<http::Request<()>, SnapshotRequestError> {
    let filters = video_ids
        .iter()
        .take(100)
        .map(|id| JsonFilter::Equal(FilterEqual::ContentId(Cow::Borrowed(id.as_str()))))
        .collect::<Box<[_]>>();
    let filter = match filters.len() {
        0 => None,
        1 => Some(serde_json::to_string(&filters[0]).map_err(SnapshotRequestError::JsonFilterSerialization)?),
        _ => Some(
            serde_json::to_string(&JsonFilter::Or {
                filters: RefOrBox::Ref(&filters),
            })
            .map_err(SnapshotRequestError::JsonFilterSerialization)?,
        ),
    };
    let query = SnapshotSearchQuery {
        q: SnapshotSearchQueryTarget::None,
        fields: SEARCH_FIELDS,
        json_filter: filter,
        sort: "startTime",
        limit: video_ids.len().min(100),
        offset: 0,
        context: app_name,
    };
    build_request(app_name, query)
}

fn build_request(app_name: &str, query: SnapshotSearchQuery<'_>) -> Result<http::Request<()>, SnapshotRequestError> {
    let q = serde_urlencoded::to_string(&query)?;
    let uri = format!("{SNAPSHOT_URL}?{q}");

    http::Request::builder()
        .method(http::Method::GET)
        .uri(uri)
        .header(USER_AGENT, app_name)
        .body(())
        .map_err(Into::into)
}

fn sort_value(sort_field: &SortField, sort_direction: SortDirection) -> String {
    let direction = match sort_direction {
        SortDirection::Asc => "+",
        SortDirection::Desc => "-",
    };
    let field = match sort_field {
        SortField::ViewCounter => "viewCounter",
        SortField::MylistCounter => "mylistCounter",
        SortField::LengthSeconds => "lengthSeconds",
        SortField::StartTime => "startTime",
        SortField::CommentCounter => "commentCounter",
        SortField::LastCommentTime => "lastCommentTime",
    };
    format!("{direction}{field}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::ops::Bound::{Excluded, Included};

    const TEST_APP_NAME: &str = "test-nico-random-picker";

    fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
        params.iter().find_map(|(name, value)| (name == key).then_some(value.as_str()))
    }

    #[test]
    fn build_search_request_encodes_snapshot_query_with_json_filter() {
        let criteria = SearchCriteria {
            tag_name: "foo bar".to_string(),
            view_count_min: Some(100),
            view_count_max: Some(200),
            start_time_from: Some(OffsetDateTime::parse("2024-01-02T03:04:00+09:00", &Rfc3339).unwrap()),
            start_time_to: Some(OffsetDateTime::parse("2024-01-03T03:04:00+09:00", &Rfc3339).unwrap()),
        };

        let request = build_search_request(TEST_APP_NAME, &criteria, &SortField::ViewCounter, SortDirection::Asc, 3, 42).unwrap();
        let uri = request.uri();
        let params = serde_urlencoded::from_str::<Vec<(String, String)>>(uri.query().unwrap()).unwrap();
        let json_filter = serde_json::from_str::<serde_json::Value>(param(&params, "jsonFilter").unwrap()).unwrap();

        assert_eq!(uri.path(), "/api/v2/snapshot/video/contents/search");
        assert_eq!(param(&params, "q").unwrap(), "foo bar");
        assert_eq!(param(&params, "targets").unwrap(), "tagsExact");
        assert_eq!(param(&params, "fields").unwrap(), SEARCH_FIELDS);
        assert_eq!(param(&params, "_sort").unwrap(), "+viewCounter");
        assert_eq!(param(&params, "_limit").unwrap(), "3");
        assert_eq!(param(&params, "_offset").unwrap(), "42");
        assert_eq!(param(&params, "_context").unwrap(), TEST_APP_NAME);
        assert_eq!(request.headers().get(USER_AGENT).unwrap(), TEST_APP_NAME);
        assert_eq!(
            json_filter,
            json!({
                "type": "and",
                "filters": [
                    {
                        "type": "range",
                        "field": "viewCounter",
                        "from": 100,
                        "include_lower": true,
                        "to": 200,
                        "include_upper": true
                    },
                    {
                        "type": "range",
                        "field": "startTime",
                        "from": "2024-01-02T03:04:00+09:00",
                        "include_lower": true,
                        "to": "2024-01-03T03:04:00+09:00",
                        "include_upper": true
                    }
                ]
            })
        );
    }

    #[test]
    fn build_search_request_allows_one_sided_ranges() {
        let criteria = SearchCriteria {
            tag_name: "foo bar".to_string(),
            view_count_min: Some(100),
            view_count_max: None,
            start_time_from: None,
            start_time_to: None,
        };

        let request = build_search_request(TEST_APP_NAME, &criteria, &SortField::ViewCounter, SortDirection::Asc, 3, 42).unwrap();
        let params = serde_urlencoded::from_str::<Vec<(String, String)>>(request.uri().query().unwrap()).unwrap();
        let json_filter = serde_json::from_str::<serde_json::Value>(param(&params, "jsonFilter").unwrap()).unwrap();

        assert_eq!(
            json_filter,
            json!({
                "type": "and",
                "filters": [
                    {
                        "type": "range",
                        "field": "viewCounter",
                        "from": 100,
                        "include_lower": true
                    }
                ]
            })
        );
    }

    #[test]
    fn build_get_details_request_encodes_content_id_filter() {
        let ids = vec!["sm9".to_string(), "so123".to_string()];
        let request = build_get_details_request(TEST_APP_NAME, &ids).unwrap();
        let uri = request.uri();
        let params = serde_urlencoded::from_str::<Vec<(String, String)>>(uri.query().unwrap()).unwrap();
        let json_filter = serde_json::from_str::<serde_json::Value>(param(&params, "jsonFilter").unwrap()).unwrap();

        assert_eq!(uri.path(), "/api/v2/snapshot/video/contents/search");
        assert_eq!(param(&params, "q").unwrap(), "");
        assert!(param(&params, "targets").is_none());
        assert_eq!(param(&params, "fields").unwrap(), SEARCH_FIELDS);
        assert_eq!(param(&params, "_sort").unwrap(), "startTime");
        assert_eq!(param(&params, "_limit").unwrap(), "2");
        assert_eq!(param(&params, "_offset").unwrap(), "0");
        assert_eq!(param(&params, "_context").unwrap(), TEST_APP_NAME);
        assert_eq!(request.headers().get(USER_AGENT).unwrap(), TEST_APP_NAME);
        assert_eq!(
            json_filter,
            json!({
                "type": "or",
                "filters": [
                    {
                        "type": "equal",
                        "field": "contentId",
                        "value": "sm9"
                    },
                    {
                        "type": "equal",
                        "field": "contentId",
                        "value": "so123"
                    }
                ]
            })
        );
    }

    #[test]
    fn filter_range_serializes_snapshot_json_filter() {
        let filter = JsonFilter::Range(FilterRange::ViewCounter((Included(100), Excluded(200))));

        assert_eq!(
            serde_json::to_value(filter).unwrap(),
            json!({
                "type": "range",
                "field": "viewCounter",
                "from": 100,
                "include_lower": true,
                "to": 200,
                "include_upper": false
            })
        );
    }

    #[test]
    fn json_filter_equal_serializes_snapshot_json_filter() {
        let filter = JsonFilter::Equal(FilterEqual::TagsExact(Cow::Borrowed("VOCALOID")));

        assert_eq!(
            serde_json::to_value(filter).unwrap(),
            json!({
                "type": "equal",
                "field": "tagsExact",
                "value": "VOCALOID"
            })
        );
    }

    #[test]
    fn json_filter_and_serializes_nested_filters() {
        let filters = [
            JsonFilter::Equal(FilterEqual::TagsExact(Cow::Borrowed("VOCALOID"))),
            JsonFilter::Range(FilterRange::ViewCounter((Included(100), Included(200)))),
        ];
        let filter = JsonFilter::And {
            filters: RefOrBox::Ref(&filters),
        };

        assert_eq!(
            serde_json::to_value(filter).unwrap(),
            json!({
                "type": "and",
                "filters": [
                    {
                        "type": "equal",
                        "field": "tagsExact",
                        "value": "VOCALOID"
                    },
                    {
                        "type": "range",
                        "field": "viewCounter",
                        "from": 100,
                        "include_lower": true,
                        "to": 200,
                        "include_upper": true
                    }
                ]
            })
        );
    }

    #[test]
    fn json_filter_or_serializes_owned_nested_filters() {
        let filters = Box::new([
            JsonFilter::Equal(FilterEqual::GenreKeyword(Cow::Borrowed("音楽・サウンド"))),
            JsonFilter::Equal(FilterEqual::GenreKeyword(Cow::Borrowed("エンターテイメント"))),
        ]);
        let filter = JsonFilter::Or {
            filters: RefOrBox::Owned(filters),
        };

        assert_eq!(
            serde_json::to_value(filter).unwrap(),
            json!({
                "type": "or",
                "filters": [
                    {
                        "type": "equal",
                        "field": "genreKeyword",
                        "value": "音楽・サウンド"
                    },
                    {
                        "type": "equal",
                        "field": "genreKeyword",
                        "value": "エンターテイメント"
                    }
                ]
            })
        );
    }

    #[test]
    fn json_filter_not_serializes_single_nested_filter() {
        let inner = JsonFilter::Equal(FilterEqual::ContentId(Cow::Borrowed("sm9")));
        let filter = JsonFilter::Not {
            filter: RefOrBox::Ref(&inner),
        };

        assert_eq!(
            serde_json::to_value(filter).unwrap(),
            json!({
                "type": "not",
                "filter": {
                    "type": "equal",
                    "field": "contentId",
                    "value": "sm9"
                }
            })
        );
    }
}
