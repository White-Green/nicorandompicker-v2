import * as v from "valibot";
import type { SearchFormState } from "./storage";
import type { VideoContent } from "./types";
import { setTurnstileRecommended } from "./turnstileState.svelte";

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

export class ClientError extends Error {}

export type ApiResult<T> = T | Response;

export async function searchVideos(
  search: SearchFormState,
): Promise<ApiResult<VideoContent[]>> {
  const response = await fetchApi("/api/search", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(createSearchRequest(search)),
  });

  if (!response.ok) return response;
  return parseJsonApiResponse(response, searchResponseSchema);
}

export async function restoreVideoDetails(
  contentIds: string[],
): Promise<ApiResult<Record<string, VideoContent>>> {
  const response = await fetchApi("/api/restore_video_details", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(contentIds),
  });

  if (!response.ok) return response;
  return parseJsonApiResponse(response, restoreVideoDetailsResponseSchema);
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
): Promise<ApiResult<DecodedShareState>> {
  const response = await fetchApi("/api/decode_share_state", {
    method: "POST",
    headers: {
      "Content-Type": "text/plain",
    },
    body: shared,
  });
  if (!response.ok) return response;
  const decoded = await parseJsonApiResponse(
    response,
    decodeShareStateResponseSchema,
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

export async function verifyTurnstile(token: string): Promise<ApiResult<void>> {
  const response = await fetch("/api/turnstile/verify", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ token }),
  });

  if (!response.ok) return response;
}

async function fetchApi(
  input: RequestInfo | URL,
  init: RequestInit,
): Promise<Response> {
  const response = await fetch(input, init);
  setTurnstileRecommended(
    response.headers.get("NRP-Turnstile-Recommended") === "true",
  );
  return response;
}

async function parseJsonApiResponse<TSchema extends v.GenericSchema>(
  response: Response,
  schema: TSchema,
): Promise<v.InferOutput<TSchema>> {
  let value: unknown;
  try {
    value = await response.json();
  } catch (e) {
    console.error(e);
    throw new Error("JSONのパースに失敗しました。", { cause: e });
  }

  const result = v.safeParse(schema, value);
  if (result.success) {
    return result.output;
  }
  console.error("API response validation failed", {
    issues: result.issues,
    responseStatus: response.status,
  });
  throw new Error("APIのレスポンスが不正です。");
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

export async function apiRequestWithRetry<T>(
  request: () => Promise<ApiResult<T>>,
  turnstileVerify: () => Promise<void>,
): Promise<T> {
  while (true) {
    const response = await request();
    if (!(response instanceof Response)) {
      return response;
    } else if (response.status === 429) {
      const waitTime = Number.parseInt(
        response.headers.get("Retry-After") ?? "",
      );
      const wait = new Promise<void>((resolve) =>
        setTimeout(
          resolve,
          Number.isFinite(waitTime) ? waitTime * 1000 : 10_000,
        ),
      );
      const turnstileRecommended =
        response.headers.get("NRP-Turnstile-Recommended") === "true";
      await Promise.race(
        turnstileRecommended ? [turnstileVerify(), wait] : [wait],
      );
    } else if (400 <= response.status && response.status < 500) {
      const errorMessage = await responseErrorMessage(response);
      console.error("API client error", { status: response.status });
      throw new ClientError(errorMessage);
    } else {
      const errorMessage = await responseErrorMessage(response);
      console.error("API server error", { status: response.status });
      throw new Error(errorMessage);
    }
  }
}
