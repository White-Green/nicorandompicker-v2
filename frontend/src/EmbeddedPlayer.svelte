<script lang="ts">
  import { onMount, tick } from "svelte";
  import { notify } from "./notifications.svelte";
  import type { PlayerLoopType } from "./storage";
  import type { VideoContent, VideoPlayingData } from "./types";
  import loopButton from "./assets/loop_button.svg";
  import loopNoneButton from "./assets/loop_none_button.svg";
  import loopOnceButton from "./assets/loop_once_button.svg";
  import triangleLeft from "./assets/triangle_left.svg";
  import triangleRight from "./assets/triangle_right.svg";
  import copyClipboard from "./assets/copy_clipboard_button.svg";

  interface Props {
    selectedVideo: VideoPlayingData | null;
    loopType: PlayerLoopType;
    videos: VideoContent[];
    enabled: boolean;
    onClose: () => void;
  }

  let {
    selectedVideo = $bindable(),
    loopType = $bindable(),
    videos,
    enabled,
    onClose,
  }: Props = $props();

  let readyToPlay = $state(false);
  let playerFrame = $state<HTMLIFrameElement | null>(null);

  onMount(() => {
    window.addEventListener("message", handlePlayerMessage);
    return () => window.removeEventListener("message", handlePlayerMessage);
  });

  function next() {
    if (!selectedVideo || videos.length === 0) return;
    const index = videos.findIndex(
      (video) => video.contentId === selectedVideo?.contentId,
    );
    const nextVideo =
      index !== -1 ? videos[(index + 1) % videos.length] : videos[0];
    selectedVideo = toVideoPlayingData(nextVideo);
    if (enabled) {
      scrollSelectedIntoView();
    }
  }

  function prev() {
    if (!selectedVideo || videos.length === 0) return;
    const index = videos.findIndex(
      (video) => video.contentId === selectedVideo?.contentId,
    );
    const prevVideo =
      index !== -1
        ? videos[(index + videos.length - 1) % videos.length]
        : videos[videos.length - 1];
    selectedVideo = toVideoPlayingData(prevVideo);
    if (enabled) {
      scrollSelectedIntoView();
    }
  }

  async function scrollSelectedIntoView() {
    await tick();
    if (!selectedVideo) return;
    document
      .getElementById(`video-${selectedVideo.contentId}`)
      ?.scrollIntoView({
        behavior: "smooth",
        block: "center",
        inline: "center",
      });
  }

  function handlePlayerMessage(event: MessageEvent) {
    if (event.origin !== "https://embed.nicovideo.jp") return;
    if (event.data?.eventName === "loadComplete" && readyToPlay) {
      readyToPlay = false;
      postPlayerEvent("play");
    }
    if (
      event.data?.eventName === "playerStatusChange" &&
      event.data?.data?.playerStatus === 4
    ) {
      if (loopType === "Loop") {
        readyToPlay = true;
        next();
      } else if (loopType === "LoopOne") {
        postPlayerEvent("play");
      }
    }
  }

  function postPlayerEvent(eventName: string) {
    playerFrame?.contentWindow?.postMessage(
      { sourceConnectorType: 1, eventName },
      "https://embed.nicovideo.jp",
    );
  }

  function cycleLoopType() {
    loopType =
      loopType === "Loop"
        ? "LoopOne"
        : loopType === "LoopOne"
          ? "None"
          : "Loop";
  }

  function toVideoPlayingData(video: VideoContent): VideoPlayingData {
    return { contentId: video.contentId, tags: video.tags };
  }

  async function copyTag(tag: string) {
    try {
      await navigator.clipboard.writeText(tag);
      notify("success", `タグ「${tag}」をコピーしました`);
    } catch (reason) {
      console.error(reason);
      notify("error", "タグのコピーに失敗しました");
    }
  }
</script>

<aside
  class="grid min-h-0 w-full content-end grid-rows-[auto_3rem_4rem] gap-4 p-4 xl:h-full xl:w-auto xl:max-w-full xl:grid-rows-[minmax(0,1fr)_3rem_4rem]"
>
  {#if selectedVideo}
    <div class="grid min-h-0 content-end xl:max-w-full">
      <iframe
        bind:this={playerFrame}
        class="aspect-video w-full border-0 xl:max-w-full"
        title="nicovideo_player"
        src={`https://embed.nicovideo.jp/watch/${selectedVideo.contentId}?jsapi=1`}
        allowfullscreen
      ></iframe>
    </div>
  {/if}
  <div class="grid grid-cols-[1fr_auto_auto_auto_1fr] gap-1">
    <button
      class="btn btn-primary prev_button col-start-2"
      title="前の動画へ"
      onclick={prev}
    >
      <img alt="prev video" class="h-4" src={triangleLeft} />
    </button>
    <button
      class="btn btn-primary loop_type_button"
      title={loopType === "Loop"
        ? "全動画ループ中 / 1動画ループへ変更"
        : loopType === "LoopOne"
          ? "1動画ループ中 / 連続再生無しへ変更"
          : "連続再生無し / 全動画ループへ変更"}
      onclick={cycleLoopType}
    >
      {#if loopType === "Loop"}
        <img
          alt="video loop state is loop-all-video"
          class="h-4"
          src={loopButton}
        />
      {:else if loopType === "LoopOne"}
        <img
          alt="video loop state is loop-one-video"
          class="h-4"
          src={loopOnceButton}
        />
      {:else}
        <img
          alt="video loop state is no-loop"
          class="h-4"
          src={loopNoneButton}
        />
      {/if}
    </button>
    <button
      class="btn btn-primary next_button"
      title="次の動画へ"
      onclick={next}
    >
      <img alt="next video" class="h-4" src={triangleRight} />
    </button>
    <button
      class="btn btn-primary close_button justify-self-end"
      title="埋め込みプレーヤーを閉じる"
      onclick={onClose}
    >
      閉じる
    </button>
  </div>
  <div class="h-16 overflow-y-auto">
    {#if selectedVideo}
      {#each selectedVideo.tags as playerTag (playerTag)}
        <button
          class="badge badge-soft badge-primary m-1"
          title="タグをコピー"
          onclick={() => copyTag(playerTag)}
          ><img
            alt="copy"
            src={copyClipboard}
            class="h-4 mix-blend-difference"
          />{playerTag}</button
        >
      {/each}
    {/if}
  </div>
</aside>
