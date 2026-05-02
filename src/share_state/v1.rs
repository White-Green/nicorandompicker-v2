use crate::share_state::{ContentId, SearchState, ShareState};
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use nom::Parser;
use nom::error::{ErrorKind, ParseError};
use nom::multi::count;
use nom::number::complete::u8;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::io::Read;
use std::{io, slice};
use time::OffsetDateTime;

const VERSION: char = '1';
const BROTLI_BUFFER_SIZE: usize = 4096;
const MAX_DECOMPRESSED_SIZE: usize = 64 * 1024;
const MAX_CONTENT_IDS: u64 = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareStateV1 {
    pub search: SearchStateV1,
    pub content_ids: Vec<ContentId>,
}

impl From<ShareState> for ShareStateV1 {
    fn from(value: ShareState) -> Self {
        let ShareState { search, content_ids } = value;
        ShareStateV1 {
            search: search.into(),
            content_ids,
        }
    }
}

impl From<ShareStateV1> for ShareState {
    fn from(value: ShareStateV1) -> Self {
        let ShareStateV1 { search, content_ids } = value;
        ShareState {
            search: search.into(),
            content_ids,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchStateV1 {
    pub tag: String,
    pub uploaded_since: Option<OffsetDateTime>,
    pub uploaded_until: Option<OffsetDateTime>,
    pub view_min: Option<u64>,
    pub view_max: Option<u64>,
    pub result_count: u64,
}

impl From<SearchState> for SearchStateV1 {
    fn from(value: SearchState) -> Self {
        let SearchState {
            tag,
            uploaded_since,
            uploaded_until,
            view_min,
            view_max,
            result_count,
        } = value;
        SearchStateV1 {
            tag,
            uploaded_since,
            uploaded_until,
            view_min,
            view_max,
            result_count,
        }
    }
}

impl From<SearchStateV1> for SearchState {
    fn from(value: SearchStateV1) -> Self {
        let SearchStateV1 {
            tag,
            uploaded_since,
            uploaded_until,
            view_min,
            view_max,
            result_count,
        } = value;
        SearchState {
            tag,
            uploaded_since,
            uploaded_until,
            view_min,
            view_max,
            result_count,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("共有データが空です")]
    Empty,
    #[error("未対応の共有データバージョンです: {0}")]
    UnsupportedVersion(char),
    #[error("デコードに失敗しました: {self:?}")]
    Base64(#[from] base64::DecodeError),
    #[error("デコードに失敗しました: {self:?}")]
    Decompress(#[from] io::Error),
    #[error("共有データの展開後サイズが大きすぎます: limit={limit} bytes")]
    DecompressedSizeLimitExceeded { limit: usize },
    #[error("デコードに失敗しました: {self:?}")]
    InvalidEscapeSequence,
    #[error("デコードに失敗しました: {self:?}")]
    TrailingBytes,
    #[error("デコードに失敗しました: {self:?}")]
    UnexpectedEof,
    #[error("デコードに失敗しました: {self:?}")]
    Utf8(std::string::FromUtf8Error),
    #[error("デコードに失敗しました: {self:?}")]
    Timestamp(time::error::ComponentRange),
    #[error("デコードに失敗しました: {self:?}")]
    InvalidContentIdPrefix,
    #[error("デコードに失敗しました: {self:?}")]
    InvalidVarint,
    #[error("共有データのcontentIdsが多すぎます: limit={limit}")]
    TooManyContentIds { limit: u64 },
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("共有データの圧縮に失敗しました")]
    Compress,
    #[error("共有データのエンコードに失敗しました: contentIdsが多すぎます")]
    TooManyContentIds,
}

pub fn encode(state: &ShareStateV1) -> Result<String, EncodeError> {
    let raw = encode_binary(state)?;
    let compressed = brotli_compress(&raw)?;
    Ok(format!("{VERSION}{}", BASE64_URL_SAFE_NO_PAD.encode(compressed)))
}

pub fn decode(encoded: &str) -> Result<ShareStateV1, DecodeError> {
    let mut chars = encoded.chars();
    let version = chars.next().ok_or(DecodeError::Empty)?;
    if version != VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }

    let payload = chars.as_str();
    let compressed = BASE64_URL_SAFE_NO_PAD.decode(payload)?;
    let raw = brotli_decompress(&compressed)?;
    decode_binary(&raw)
}

fn encode_binary(state: &ShareStateV1) -> Result<Vec<u8>, EncodeError> {
    let mut bytes = Vec::new();
    write_bytes(&mut bytes, state.search.tag.as_bytes());
    write_varint(&mut bytes, state.search.result_count);

    let flags = (state.search.view_min.is_some() as u8)
        | ((state.search.view_max.is_some() as u8) << 1)
        | ((state.search.uploaded_since.is_some() as u8) << 2)
        | ((state.search.uploaded_until.is_some() as u8) << 3);
    bytes.push(flags);

    if let Some(value) = state.search.view_min {
        write_varint(&mut bytes, value);
    }
    if let Some(value) = state.search.view_max {
        write_varint(&mut bytes, value);
    }
    if let Some(value) = state.search.uploaded_since {
        write_varint(&mut bytes, encode_signed(value.unix_timestamp()));
    }
    if let Some(value) = state.search.uploaded_until {
        write_varint(&mut bytes, encode_signed(value.unix_timestamp()));
    }

    if state.content_ids.is_empty() {
        write_varint(&mut bytes, 0);
        return Ok(bytes);
    }

    if state.content_ids.len() > MAX_CONTENT_IDS as usize {
        return Err(EncodeError::TooManyContentIds);
    }

    let mut prefixes = HashMap::<&str, usize>::new();
    for id in state.content_ids.iter() {
        *prefixes.entry(id.prefix.as_str()).or_default() += 1;
    }
    let mut prefixes = prefixes.into_iter().collect::<Vec<_>>();
    prefixes.sort_unstable_by_key(|&(_, count)| Reverse(count));
    prefixes.iter_mut().enumerate().for_each(|(i, (_, j))| *j = i);
    assert!(prefixes.len() <= 255);
    bytes.push(prefixes.len() as u8);
    for (prefix, _) in &prefixes {
        write_bytes(&mut bytes, prefix.as_bytes());
    }
    let prefixes = HashMap::<&str, usize>::from_iter(prefixes);

    write_varint(&mut bytes, state.content_ids.len() as u64);
    for id in &state.content_ids {
        let &prefix = prefixes.get(id.prefix.as_str()).unwrap();
        write_varint(&mut bytes, prefix as u64);
        write_varint(&mut bytes, id.number);
    }

    Ok(bytes)
}

fn decode_binary(bytes: &[u8]) -> Result<ShareStateV1, DecodeError> {
    match parse_share_state(bytes) {
        Ok(([], state)) => Ok(state),
        Ok((_, _)) => Err(DecodeError::TrailingBytes),
        Err(nom::Err::Error(err) | nom::Err::Failure(err)) => Err(err),
        Err(nom::Err::Incomplete(_)) => Err(DecodeError::UnexpectedEof),
    }
}

fn parse_share_state(input: &[u8]) -> ParseResult<'_, ShareStateV1> {
    let (input, tag) = parse_string(input)?;
    let (input, result_count) = parse_varint(input)?;
    let (input, flags) = u8(input)?;

    let (input, view_min) = parse_optional_varint(input, flags & 0b0001 != 0)?;
    let (input, view_max) = parse_optional_varint(input, flags & 0b0010 != 0)?;
    let (input, uploaded_since) = parse_optional_datetime(input, flags & 0b0100 != 0)?;
    let (input, uploaded_until) = parse_optional_datetime(input, flags & 0b1000 != 0)?;
    let (input, prefix_count) = u8(input)?;
    if prefix_count == 0 {
        if !input.is_empty() {
            return Err(nom::Err::Failure(DecodeError::TrailingBytes));
        }
        Ok((
            input,
            ShareStateV1 {
                search: SearchStateV1 {
                    tag,
                    uploaded_since,
                    uploaded_until,
                    view_min,
                    view_max,
                    result_count,
                },
                content_ids: Vec::new(),
            },
        ))
    } else {
        let (input, prefixes) = count(parse_string, prefix_count as usize).parse(input)?;
        let (input, content_id_count) = parse_varint(input)?;
        if content_id_count > MAX_CONTENT_IDS {
            return Err(nom::Err::Failure(DecodeError::TooManyContentIds { limit: MAX_CONTENT_IDS }));
        }
        let (input, content_ids) = count(parse_content_id(&prefixes), content_id_count as usize).parse(input)?;
        if !input.is_empty() {
            return Err(nom::Err::Failure(DecodeError::TrailingBytes));
        }
        Ok((
            input,
            ShareStateV1 {
                search: SearchStateV1 {
                    tag,
                    uploaded_since,
                    uploaded_until,
                    view_min,
                    view_max,
                    result_count,
                },
                content_ids,
            },
        ))
    }
}

fn write_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    for &b in bytes.iter() {
        match b {
            0 => output.extend([b'\\', b'0']),
            b'\\' => output.extend([b'\\', b'\\']),
            b => output.push(b),
        }
    }
    output.push(0);
}

fn parse_bytes(input: &[u8]) -> ParseResult<'_, Vec<u8>> {
    let (input, bytes) = nom::bytes::take_until(slice::from_ref(&0u8)).parse(input)?;
    let (input, _) = nom::bytes::tag(slice::from_ref(&0u8)).parse(input)?;
    let mut result = Vec::with_capacity(bytes.len());
    let mut bytes = bytes.iter().copied();
    while let Some(b) = bytes.next() {
        match b {
            b'\\' => {
                let Some(next_byte) = bytes.next() else {
                    return Err(nom::Err::Failure(DecodeError::InvalidEscapeSequence));
                };
                match next_byte {
                    b'0' => result.push(0),
                    b'\\' => result.push(b'\\'),
                    _ => return Err(nom::Err::Failure(DecodeError::InvalidEscapeSequence)),
                }
            }
            b => result.push(b),
        }
    }
    Ok((input, result))
}

fn write_varint(output: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        output.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

fn parse_varint(input: &[u8]) -> ParseResult<'_, u64> {
    let mut input = input;
    let mut value = 0_u64;
    let mut shift = 0;

    loop {
        let (rest, byte) = u8(input)?;
        input = rest;
        if shift >= u64::BITS && byte != 0 {
            return Err(nom::Err::Failure(DecodeError::InvalidVarint));
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok((input, value));
        }
        shift += 7;
        if shift >= u64::BITS {
            return Err(nom::Err::Failure(DecodeError::InvalidVarint));
        }
    }
}

fn encode_signed(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

fn decode_signed(value: u64) -> i64 {
    ((value >> 1) as i64) ^ (-((value & 1) as i64))
}

fn brotli_compress(bytes: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let mut output = Vec::new();
    let params = brotli::enc::BrotliEncoderParams {
        quality: 11,
        ..Default::default()
    };
    brotli::BrotliCompress(&mut io::Cursor::new(bytes), &mut output, &params).map_err(|_| EncodeError::Compress)?;
    Ok(output)
}

fn brotli_decompress(bytes: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let mut output = Vec::new();
    brotli::Decompressor::new(io::Cursor::new(bytes), BROTLI_BUFFER_SIZE)
        .take(MAX_DECOMPRESSED_SIZE as u64 + 1)
        .read_to_end(&mut output)?;
    if output.len() > MAX_DECOMPRESSED_SIZE {
        return Err(DecodeError::DecompressedSizeLimitExceeded {
            limit: MAX_DECOMPRESSED_SIZE,
        });
    }
    Ok(output)
}

type ParseResult<'a, T> = nom::IResult<&'a [u8], T, DecodeError>;

impl ParseError<&[u8]> for DecodeError {
    fn from_error_kind(_input: &[u8], kind: ErrorKind) -> Self {
        if kind == ErrorKind::Eof {
            DecodeError::UnexpectedEof
        } else {
            DecodeError::TrailingBytes
        }
    }

    fn append(_input: &[u8], _kind: ErrorKind, other: Self) -> Self {
        other
    }
}

fn parse_string(input: &[u8]) -> ParseResult<'_, String> {
    let (input, bytes) = parse_bytes(input)?;
    String::from_utf8(bytes)
        .map(|value| (input, value))
        .map_err(|err| nom::Err::Failure(DecodeError::Utf8(err)))
}

fn parse_optional_varint(input: &[u8], present: bool) -> ParseResult<'_, Option<u64>> {
    if present {
        parse_varint(input).map(|(input, value)| (input, Some(value)))
    } else {
        Ok((input, None))
    }
}

fn parse_optional_datetime(input: &[u8], present: bool) -> ParseResult<'_, Option<OffsetDateTime>> {
    if !present {
        return Ok((input, None));
    }

    let (input, value) = parse_varint(input)?;
    OffsetDateTime::from_unix_timestamp(decode_signed(value))
        .map(|value| (input, Some(value)))
        .map_err(|err| nom::Err::Failure(DecodeError::Timestamp(err)))
}

fn parse_content_id(prefixes: &[String]) -> impl Fn(&[u8]) -> ParseResult<'_, ContentId> {
    move |input| {
        let (input, prefix_code) = u8(input)?;
        let prefix = prefixes
            .get(prefix_code as usize)
            .ok_or(nom::Err::Failure(DecodeError::InvalidContentIdPrefix))?;
        let (input, number) = parse_varint(input)?;
        Ok((
            input,
            ContentId {
                prefix: prefix.clone(),
                number,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::collection::vec;
    use proptest::prelude::*;
    use time::format_description::well_known::Rfc3339;
    use time::macros::datetime;

    #[test]
    fn roundtrips_share_state() {
        let state = ShareStateV1 {
            search: SearchStateV1 {
                tag: "歌うボイスロイド OR VOICEVOX".to_owned(),
                uploaded_since: Some(datetime!(2020-01-01 00:00 UTC)),
                uploaded_until: Some(datetime!(2024-12-31 23:59:59 UTC)),
                view_min: Some(10_000),
                view_max: Some(3_000_000),
                result_count: 100,
            },
            content_ids: vec![
                ContentId {
                    prefix: "sm".to_owned(),
                    number: 34594493,
                },
                ContentId {
                    prefix: "so".to_owned(),
                    number: 12345678,
                },
                ContentId {
                    prefix: "ss".to_owned(),
                    number: 23456789,
                },
                ContentId {
                    prefix: "nm".to_owned(),
                    number: 8848208,
                },
                ContentId {
                    prefix: "lv".to_owned(),
                    number: 123,
                },
            ],
        };

        let encoded = encode(&state).unwrap();

        assert!(encoded.starts_with(VERSION));
        assert_eq!(decode(&encoded).unwrap(), state);
    }

    #[test]
    fn decode_rejects_unsupported_version() {
        assert!(matches!(decode("2abc"), Err(DecodeError::UnsupportedVersion('2'))));
    }

    #[test]
    fn decode_rejects_oversized_decompressed_payload() {
        let state = ShareStateV1 {
            search: SearchStateV1 {
                tag: "a".repeat(MAX_DECOMPRESSED_SIZE + 1),
                uploaded_since: None,
                uploaded_until: None,
                view_min: None,
                view_max: None,
                result_count: 100,
            },
            content_ids: Vec::new(),
        };
        let encoded = encode(&state).unwrap();

        assert!(matches!(
            decode(&encoded),
            Err(DecodeError::DecompressedSizeLimitExceeded { limit }) if limit == MAX_DECOMPRESSED_SIZE
        ));
    }

    #[test]
    fn decode_rejects_too_many_content_ids() {
        let mut raw = Vec::new();
        write_bytes(&mut raw, b"tag");
        write_varint(&mut raw, 100);
        raw.push(0);
        raw.push(1);
        write_bytes(&mut raw, b"sm");
        write_varint(&mut raw, MAX_CONTENT_IDS + 1);
        for number in 1..=(MAX_CONTENT_IDS + 1) {
            write_varint(&mut raw, 0);
            write_varint(&mut raw, number);
        }
        let compressed = brotli_compress(&raw).unwrap();
        let encoded = format!("{VERSION}{}", BASE64_URL_SAFE_NO_PAD.encode(compressed));

        assert!(matches!(
            decode(&encoded),
            Err(DecodeError::TooManyContentIds { limit }) if limit == MAX_CONTENT_IDS
        ));
    }

    proptest! {
        #[test]
        fn proptest_varint(value in any::<u64>()) {
            let mut encoded = Vec::new();
            write_varint(&mut encoded, value);
            prop_assert_eq!(parse_varint(&encoded).unwrap(), (&[][..], value));
        }

        #[test]
        fn proptest_bytes(bytes in vec(any::<u8>(), 0..=100)) {
            let mut encoded = Vec::new();
            write_bytes(&mut encoded, &bytes);
            prop_assert_eq!(parse_bytes(&encoded).unwrap(), (&[][..], bytes));
        }

        #[test]
        fn proptest_signed(value in any::<i64>()) {
            let encoded = encode_signed(value);
            prop_assert_eq!(decode_signed(encoded), value);
        }

        #[test]
        fn proptest_roundtrips_share_state(state in arb_share_state()) {
            let encoded = encode(&state).unwrap();
            prop_assert_eq!(decode(&encoded).unwrap(), state);
        }
    }

    fn arb_share_state() -> impl Strategy<Value = ShareStateV1> {
        (arb_search_state(), vec(arb_content_id(), 0..=100)).prop_map(|(search, content_ids)| ShareStateV1 { search, content_ids })
    }

    fn arb_search_state() -> impl Strategy<Value = SearchStateV1> {
        (
            arb_tag(),
            prop::option::of(arb_datetime()),
            prop::option::of(arb_datetime()),
            prop::option::of(any::<u64>()),
            prop::option::of(any::<u64>()),
            any::<u64>(),
        )
            .prop_filter_map(
                "valid range",
                |(tag, uploaded_since, uploaded_until, view_min, view_max, result_count)| {
                    if uploaded_since.zip(uploaded_until).is_some_and(|(since, until)| since > until) {
                        return None;
                    }
                    if view_min.zip(view_max).is_some_and(|(min, max)| min > max) {
                        return None;
                    }
                    Some(SearchStateV1 {
                        tag,
                        uploaded_since,
                        uploaded_until,
                        view_min,
                        view_max,
                        result_count,
                    })
                },
            )
    }

    fn arb_tag() -> impl Strategy<Value = String> {
        vec(
            prop_oneof![
                Just("エンターテイメント".to_owned()),
                Just("ラジオ".to_owned()),
                Just("音楽・サウンド".to_owned()),
                Just("ダンス".to_owned()),
                Just("動物".to_owned()),
                Just("自然".to_owned()),
                Just("料理".to_owned()),
                Just("旅行・アウトドア".to_owned()),
                Just("乗り物".to_owned()),
                Just("スポーツ".to_owned()),
                Just("社会・政治・時事".to_owned()),
                Just("技術・工作".to_owned()),
                Just("解説・講座".to_owned()),
                Just("アニメ".to_owned()),
                Just("ゲーム".to_owned()),
            ],
            1..=10,
        )
        .prop_map(|tags| tags.join(if tags.len() % 2 == 0 { " OR " } else { " " }))
    }

    fn arb_content_id() -> impl Strategy<Value = ContentId> {
        (
            prop_oneof![
                Just("sm".to_owned()),
                Just("so".to_owned()),
                Just("ss".to_owned()),
                Just("nm".to_owned()),
                Just("uk".to_owned()),
            ],
            9_u64..100_000_000,
        )
            .prop_map(|(prefix, number)| ContentId { prefix, number })
    }

    fn arb_datetime() -> impl Strategy<Value = OffsetDateTime> {
        (OffsetDateTime::parse("2006-12-12T00:00:00+09:00", &Rfc3339).unwrap().unix_timestamp()
            ..OffsetDateTime::parse("2026-05-01T00:00:00+09:00", &Rfc3339).unwrap().unix_timestamp())
            .prop_map(|unix_timestamp| OffsetDateTime::from_unix_timestamp(unix_timestamp).unwrap())
    }
}
