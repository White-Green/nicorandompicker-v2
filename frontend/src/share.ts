import {
  type DecodedShareState,
  decodeShareState,
  encodeShareState,
} from "./api";
import type { SearchFormState } from "./storage";

export interface ShareState {
  search: SearchFormState;
  contentIds: string[];
}

export async function createURL(state: ShareState): Promise<string> {
  const data = await encodeShareState(state.search, state.contentIds);
  return `${window.location.origin}${window.location.pathname}?data=${encodeURIComponent(data)}`;
}

export function parseData(data: string): Promise<DecodedShareState> {
  return decodeShareState(data);
}
