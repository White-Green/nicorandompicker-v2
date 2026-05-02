export interface VideoContent {
  contentId: string;
  title: string;
  viewCounter: number;
  commentCounter: number;
  mylistCounter: number;
  likeCounter: number;
  lengthSeconds: number;
  thumbnailUrl: string;
  tags: string[];
}

export interface VideoPlayingData {
  contentId: string;
  tags: string[];
}
