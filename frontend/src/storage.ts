import * as v from "valibot";

export const appStateStorageKey = "NicoRandomPicker:state";

export type PlayerLoopType = "Loop" | "LoopOne" | "None";

export interface PersistedVideoPlayingData {
  contentId: string;
  tags: string[];
}

export interface SearchFormState {
  tag: string;
  uploadedSince: string | null;
  uploadedUntil: string | null;
  viewMin: number | null;
  viewMax: number | null;
  resultCount: number;
}

export interface PersistedState {
  version: 1;
  search: SearchFormState;
  results: {
    contentIds: string[];
    selectedVideo: PersistedVideoPlayingData | null;
  };
  player: {
    enabled: boolean;
    loopType: PlayerLoopType;
  };
}

export const defaultSearchFormState: SearchFormState = {
  tag: "",
  uploadedSince: null,
  uploadedUntil: null,
  viewMin: null,
  viewMax: null,
  resultCount: 10,
};

export const defaultPersistedState: PersistedState = {
  version: 1,
  search: defaultSearchFormState,
  results: {
    contentIds: [],
    selectedVideo: null,
  },
  player: {
    enabled: false,
    loopType: "Loop",
  },
};

const localDateTimeSchema = v.nullable(
  v.pipe(v.string(), v.regex(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/)),
);

const searchFormStateSchema = v.object({
  tag: v.string(),
  uploadedSince: localDateTimeSchema,
  uploadedUntil: localDateTimeSchema,
  viewMin: v.nullable(v.number()),
  viewMax: v.nullable(v.number()),
  resultCount: v.number(),
});

const videoPlayingDataSchema = v.object({
  contentId: v.string(),
  tags: v.array(v.string()),
});

const persistedStateSchema = v.object({
  version: v.literal(1),
  search: searchFormStateSchema,
  results: v.object({
    contentIds: v.array(v.string()),
    selectedVideo: v.nullable(videoPlayingDataSchema),
  }),
  player: v.object({
    enabled: v.boolean(),
    loopType: v.picklist(["Loop", "LoopOne", "None"]),
  }),
});

export function loadPersistedState(): PersistedState | null {
  if (typeof sessionStorage === "undefined") return null;
  return parsePersistedState(sessionStorage.getItem(appStateStorageKey));
}

export function savePersistedState(state: PersistedState) {
  if (typeof sessionStorage === "undefined") return;
  sessionStorage.setItem(appStateStorageKey, JSON.stringify(state));
}

export function parsePersistedState(raw: string | null): PersistedState | null {
  if (!raw) return null;

  try {
    const result = v.safeParse(persistedStateSchema, JSON.parse(raw));
    return result.success ? result.output : null;
  } catch {
    return null;
  }
}
