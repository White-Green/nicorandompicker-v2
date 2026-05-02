import * as v from "valibot";
import type { SearchFormState } from "./storage";
import type { VideoContent } from "./types";

interface SearchRequest {
  tag_name: string;
  video_count: number;
  view_count_min: number | null;
  view_count_max: number | null;
  start_time_from: string | null;
  start_time_to: string | null;
}

interface ShareSearchRequest {
  tag: string;
  uploadedSince: string | null;
  uploadedUntil: string | null;
  viewMin: number | null;
  viewMax: number | null;
  resultCount: number;
}

interface EncodeShareStateRequest {
  search: ShareSearchRequest;
  contentIds: string[];
}

const videoContentSchema = v.object({
  contentId: v.string(),
  title: v.string(),
  viewCounter: v.number(),
  commentCounter: v.number(),
  mylistCounter: v.number(),
  likeCounter: v.number(),
  lengthSeconds: v.number(),
  thumbnailUrl: v.string(),
  tags: v.array(v.string()),
});

const searchResponseSchema = v.array(videoContentSchema);
const restoreVideoDetailsResponseSchema = v.record(
  v.string(),
  videoContentSchema,
);
const shareSearchResponseSchema = v.object({
  tag: v.string(),
  uploadedSince: v.nullable(v.pipe(v.string(), v.isoTimestamp())),
  uploadedUntil: v.nullable(v.pipe(v.string(), v.isoTimestamp())),
  viewMin: v.nullable(v.number()),
  viewMax: v.nullable(v.number()),
  resultCount: v.number(),
});
const decodeShareStateResponseSchema = v.object({
  search: shareSearchResponseSchema,
  contents: v.array(videoContentSchema),
});

export interface DecodedShareState {
  search: SearchFormState;
  contents: VideoContent[];
}

export async function searchVideos(
  search: SearchFormState,
): Promise<VideoContent[]> {
  const response = await fetch("/api/search", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(createSearchRequest(search)),
  });

  if (!response.ok) throw new Error(await responseErrorMessage(response));
  return parseApiResponse(searchResponseSchema, await response.json());
}

export async function restoreVideoDetails(
  contentIds: string[],
): Promise<Record<string, VideoContent>> {
  const response = await fetch("/api/restore_video_details", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(contentIds),
  });

  if (!response.ok) throw new Error(await responseErrorMessage(response));
  return parseApiResponse(
    restoreVideoDetailsResponseSchema,
    await response.json(),
  );
}

export async function encodeShareState(
  search: SearchFormState,
  contentIds: string[],
): Promise<string> {
  const response = await fetch("/api/encode_share_state", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(createEncodeShareStateRequest(search, contentIds)),
  });

  if (!response.ok) throw new Error(await responseErrorMessage(response));
  return response.text();
}

export async function decodeShareState(
  shared: string,
): Promise<DecodedShareState> {
  const response = await fetch("/api/decode_share_state", {
    method: "POST",
    headers: {
      "Content-Type": "text/plain",
    },
    body: shared,
  });

  if (!response.ok) throw new Error(await responseErrorMessage(response));
  const decoded = parseApiResponse(
    decodeShareStateResponseSchema,
    await response.json(),
  );
  return {
    search: {
      tag: decoded.search.tag,
      uploadedSince: rfc3339ToLocalDateTimeOrNull(decoded.search.uploadedSince),
      uploadedUntil: rfc3339ToLocalDateTimeOrNull(decoded.search.uploadedUntil),
      viewMin: decoded.search.viewMin,
      viewMax: decoded.search.viewMax,
      resultCount: decoded.search.resultCount,
    },
    contents: decoded.contents,
  };
}

function parseApiResponse<TSchema extends v.GenericSchema>(
  schema: TSchema,
  value: unknown,
): v.InferOutput<TSchema> {
  const result = v.safeParse(schema, value);
  if (result.success) return result.output;
  console.error(result.issues);
  console.error(value);
  throw new Error("サーバから想定外の形式のレスポンスを受信しました。");
}

function createEncodeShareStateRequest(
  search: SearchFormState,
  contentIds: string[],
): EncodeShareStateRequest {
  return {
    search: {
      tag: search.tag,
      uploadedSince: localDateTimeToRfc3339OrNull(search.uploadedSince),
      uploadedUntil: localDateTimeToRfc3339OrNull(search.uploadedUntil),
      viewMin: search.viewMin,
      viewMax: search.viewMax,
      resultCount: search.resultCount,
    },
    contentIds,
  };
}

function createSearchRequest(search: SearchFormState): SearchRequest {
  return {
    tag_name: search.tag,
    video_count: search.resultCount,
    view_count_min: search.viewMin,
    view_count_max: search.viewMax,
    start_time_from: localDateTimeToRfc3339OrNull(search.uploadedSince),
    start_time_to: localDateTimeToRfc3339OrNull(search.uploadedUntil),
  };
}

function localDateTimeToRfc3339OrNull(value: string | null): string | null {
  if (value === null) return null;
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(value)) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date.toISOString();
}

function rfc3339ToLocalDateTimeOrNull(value: string | null): string | null {
  if (value === null) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  const offsetMilliseconds = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offsetMilliseconds)
    .toISOString()
    .slice(0, 16);
}

async function responseErrorMessage(response: Response): Promise<string> {
  const body = await response.text();
  try {
    const json = JSON.parse(body) as { error?: unknown };
    if (typeof json.error === "string" && json.error !== "") return json.error;
  } catch {
    if (body !== "") return body;
  }
  return `検索リクエストに失敗しました。ステータスコード: ${response.status}`;
}
