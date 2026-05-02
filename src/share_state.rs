use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::fmt::Display;
use time::OffsetDateTime;

pub mod v1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentId {
    pub prefix: String,
    pub number: u64,
}

impl Display for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.prefix, self.number)
    }
}

impl Serialize for ContentId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}{}", self.prefix, self.number))
    }
}

impl<'de> Deserialize<'de> for ContentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let content_id = String::deserialize(deserializer)?;
        let Some(split) = content_id.find(|c: char| c.is_ascii_digit()) else {
            return Err(D::Error::custom("contentId must contain a numeric part"));
        };
        if split == 0 {
            return Err(D::Error::custom("contentId must contain a prefix"));
        }

        let (prefix, number) = content_id.split_at(split);
        let number = number
            .parse()
            .map_err(|_| D::Error::custom("contentId numeric part must be an unsigned integer"))?;

        Ok(ContentId {
            prefix: prefix.to_owned(),
            number,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareState {
    pub search: SearchState,
    pub content_ids: Vec<ContentId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchState {
    pub tag: String,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub uploaded_since: Option<OffsetDateTime>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub uploaded_until: Option<OffsetDateTime>,
    pub view_min: Option<u64>,
    pub view_max: Option<u64>,
    pub result_count: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error(transparent)]
    V1(v1::EncodeError),
}

pub fn encode(state: ShareState) -> Result<String, EncodeError> {
    v1::encode(&state.into()).map_err(EncodeError::V1)
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error(transparent)]
    V1(v1::DecodeError),
}

pub fn decode(encoded: &str) -> Result<ShareState, DecodeError> {
    v1::decode(encoded).map_err(DecodeError::V1).map(From::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_id_deserializes_from_string() {
        let content_id = serde_json::from_value::<ContentId>(serde_json::json!("sm34594493")).unwrap();

        assert_eq!(
            content_id,
            ContentId {
                prefix: "sm".to_owned(),
                number: 34_594_493,
            }
        );
    }

    #[test]
    fn content_id_rejects_invalid_strings() {
        assert!(serde_json::from_value::<ContentId>(serde_json::json!("sm")).is_err());
        assert!(serde_json::from_value::<ContentId>(serde_json::json!("12345")).is_err());
        assert!(serde_json::from_value::<ContentId>(serde_json::json!("sm12x")).is_err());
    }
}
