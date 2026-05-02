#![allow(unused)]
use serde::{Deserialize, Serialize};

pub enum SnapshotResponse {
    Ok(OkResponse),
    Error(ErrorResponse),
}

pub fn parse_snapshot_response(response: http::Response<Vec<u8>>) -> Result<SnapshotResponse, serde_json::Error> {
    if response.status().is_success() {
        serde_json::from_slice::<OkResponse>(response.body()).map(SnapshotResponse::Ok)
    } else {
        serde_json::from_slice::<ErrorResponse>(response.body()).map(SnapshotResponse::Error)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSearchResult {
    pub content_id: String,
    pub title: String,
    pub view_counter: u64,
    pub comment_counter: u64,
    pub mylist_counter: u64,
    pub like_counter: u64,
    pub length_seconds: u64,
    pub thumbnail_url: String,
    #[serde(deserialize_with = "deserialize_tags")]
    pub tags: Vec<String>,
}

fn deserialize_tags<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let tags = String::deserialize(deserializer)?;
    Ok(tags.split_whitespace().map(ToString::to_string).collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkMeta {
    pub status: u16,
    pub total_count: usize,
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMeta {
    pub status: u16,
    pub error_code: String,
    pub error_message: String,
}

#[derive(Debug, Deserialize)]
pub struct OkResponse {
    pub meta: OkMeta,
    pub data: Vec<VideoSearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub meta: ErrorMeta,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_response() {
        let response_ok = json!({
          "meta": {
            "status": 200,
            "totalCount": 1,
            "id": "594513df-85ea-4122-9859-f4ec2701cacf"
          },
          "data": [
            {
              "contentId": "sm9",
              "title": "テスト",
              "viewCounter": 1,
              "commentCounter": 10,
              "mylistCounter": 100,
              "likeCounter": 1000,
              "lengthSeconds": 1000,
              "thumbnailUrl": "https://thumbnail.example.com/sm9",
              "tags": "a b c",
            }
          ]
        });
        let response_bad_request = json!({
          "meta": {
            "status": 400,
            "errorCode": "QUERY_PARSE_ERROR",
            "errorMessage": "query parse error"
          }
        });
        let response_internal_server_error = json!({
          "meta": {
            "status": 500,
            "errorCode": "INTERNAL_SERVER_ERROR",
            "errorMessage": "please retry later."
          }
        });
        let response_maintenance = json!({
          "meta": {
            "status": 503,
            "errorCode": "MAINTENANCE",
            "errorMessage": "please retry later."
          }
        });

        assert!(serde_json::from_str::<OkResponse>(&serde_json::to_string(&response_ok).unwrap()).is_ok());
        assert!(serde_json::from_str::<ErrorResponse>(&serde_json::to_string(&response_bad_request).unwrap()).is_ok());
        assert!(serde_json::from_str::<ErrorResponse>(&serde_json::to_string(&response_internal_server_error).unwrap()).is_ok());
        assert!(serde_json::from_str::<ErrorResponse>(&serde_json::to_string(&response_maintenance).unwrap()).is_ok());
    }
}
