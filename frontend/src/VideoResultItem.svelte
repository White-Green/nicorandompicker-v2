<script lang="ts">
  import type { VideoContent } from "./types";
  import deleteButton from "./assets/delete_button.svg";
  import playButton from "./assets/play_button.svg";

  interface Props {
    video: VideoContent;
    selected: boolean;
    onPlay: (video: VideoContent) => void;
    onDelete: (contentId: string) => void;
  }

  let { video, selected, onPlay, onDelete }: Props = $props();
  let contentElement = $state<HTMLDivElement | null>(null);
  let suppressNextActionClick = false;

  function unescapeHTML(text: string): string {
    const textarea = document.createElement("textarea");
    textarea.innerHTML = text;
    return textarea.textContent || "";
  }

  function duration(total: number): string {
    return `${Math.floor(total / 60)}:${(total % 60).toString().padStart(2, "0")}`;
  }

  function handleContentPointerDown(event: PointerEvent) {
    if (event.pointerType === "mouse") return;
    if (event.target instanceof Element && event.target.closest("button,a"))
      return;
    if (!isActionVisible()) {
      suppressNextActionClick = true;
    }
    contentElement?.focus({ preventScroll: true });
  }

  function handleContentClickCapture(event: MouseEvent) {
    if (event.target instanceof Element && event.target.closest("button,a"))
      return;
    suppressNextActionClick = false;
  }

  function shouldSuppressAction(event: MouseEvent): boolean {
    if (suppressNextActionClick) {
      event.preventDefault();
      event.stopPropagation();
      suppressNextActionClick = false;
      return true;
    }
    return false;
  }

  function handleDelete(event: MouseEvent) {
    if (shouldSuppressAction(event)) return;
    onDelete(video.contentId);
  }

  function handlePlay(event: MouseEvent) {
    if (shouldSuppressAction(event)) return;
    onPlay(video);
  }

  function isActionVisible(): boolean {
    return (
      document.activeElement instanceof Element &&
      !!contentElement?.contains(document.activeElement)
    );
  }
</script>

<div
  id={`video-${video.contentId}`}
  class={`video_link_component ${selected ? "video_link_component_selected " : ""}`}
>
  <img
    alt={`thumbnail of ${video.contentId}`}
    src={`${video.thumbnailUrl}.M`}
    loading="lazy"
    onerror={(event) =>
      ((event.currentTarget as HTMLImageElement).src = video.thumbnailUrl)}
  />
  <div
    bind:this={contentElement}
    class="video_link_content"
    role="group"
    tabindex="-1"
    onpointerdown={handleContentPointerDown}
    onclickcapture={handleContentClickCapture}
  >
    <div class="video_link_title_area">
      <button
        class="video_link_delete_area"
        onclick={handleDelete}
        title="検索結果から削除"
      >
        <img
          alt={`delete ${video.contentId} from search result`}
          src={deleteButton}
          width="30"
          height="30"
        />
      </button>
      <a
        href={`https://nico.ms/${video.contentId}`}
        target="_blank"
        rel="noreferrer noopener">{unescapeHTML(video.title)}</a
      >
    </div>
    <div class="video_link_detail_area">
      <div class="video_link_detail">
        <span>▶️{video.viewCounter.toLocaleString()}</span>
        <span>💬{video.commentCounter.toLocaleString()}</span>
        <span>📁{video.mylistCounter.toLocaleString()}</span>
        <span>♥️{video.likeCounter.toLocaleString()}</span>
      </div>
      <div class="video_link_duration">{duration(video.lengthSeconds)}</div>
    </div>
    <button
      class="video_link_play_logo"
      onclick={handlePlay}
      title={`${video.contentId} を再生`}
    >
      <img alt={`play ${video.contentId}`} src={playButton} />
    </button>
  </div>
</div>

<style>
  .video_link_component {
    background-size: contain;
    font-size: 1.2em;
    color: whitesmoke;
    text-shadow:
      1px 0 #202020,
      -1px 0 #202020,
      0 1px #202020,
      0 -1px #202020,
      1px 1px #202020,
      -1px 1px #202020,
      -1px 1px #202020,
      -1px -1px #202020,
      2px 0 #202020,
      -2px 0 #202020,
      0 2px #202020,
      0 -2px #202020;
    border-radius: 10px;
    aspect-ratio: 16 / 9;
    position: relative;
    overflow: hidden;
    box-shadow: #202020 3px 3px 2px;
    transition-duration: 0.2s;
  }

  .video_link_component_selected {
    box-shadow: tomato 8px 8px 5px;
    transition-duration: 0.2s;
  }

  .video_link_component_selected::before {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 8px;
    background: tomato;
  }

  .video_link_component > img {
    position: absolute;
    height: 100%;
    left: 0;
    right: 0;
    margin: auto;
  }

  .video_link_content {
    position: relative;
    height: 100%;
    border-radius: 8px;
  }

  .video_link_content:hover {
    background: #2020207f;
  }

  .video_link_content:not(:hover) > .video_link_play_logo,
  .video_link_content:not(:hover) .video_link_detail {
    display: none;
  }

  .video_link_play_logo {
    display: block;
    position: absolute;
    top: 35%;
    bottom: 35%;
    right: 35%;
    left: 35%;
    margin: auto;
    text-align: center;
    border: none;
    background: none;
  }

  .video_link_play_logo > img {
    height: 100%;
    mix-blend-mode: multiply;
    transition-duration: 0.3s;
  }

  .video_link_play_logo:hover > img {
    transition-duration: 0.3s;
    transform: scale(1.2);
  }

  .video_link_title_area {
    padding: 5px;
  }

  .video_link_title_area > a {
    color: inherit;
    text-decoration: none;
  }

  .video_link_component:hover .video_link_title_area > a {
    background: transparent;
  }

  .video_link_delete_area {
    float: right;
    transition-duration: 0.2s;
    border: none;
    background: none;
  }

  .video_link_delete_area:hover {
    transform: scale(1.1);
    transition-duration: 0.2s;
  }

  .video_link_content:not(:hover) .video_link_delete_area {
    visibility: hidden;
  }

  @media (hover: none) {
    .video_link_content:focus-within {
      background: #2020207f;
    }

    .video_link_content:focus-within > .video_link_play_logo,
    .video_link_content:focus-within .video_link_detail {
      display: block;
    }

    .video_link_content:focus-within .video_link_delete_area {
      visibility: visible;
    }
  }

  .video_link_detail_area {
    position: absolute;
    display: grid;
    grid-template-columns: auto 10px 1fr auto;
    grid-template-rows: 1fr auto;
    bottom: 5px;
    left: 5px;
    right: 5px;
    line-height: 1.6em;
  }

  .video_link_detail {
    grid-row: 1 / 3;
    text-align: center;
  }

  .video_link_detail > span {
    display: inline-block;
    margin: 0 3px;
  }

  .video_link_duration {
    grid-column: 4 / 5;
    grid-row: 2 / 3;
    padding: 0 8px;
    border-radius: 5px;
    background: #2020207f;
  }
</style>
