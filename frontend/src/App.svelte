<script lang="ts">
  import { onMount, tick } from "svelte";
  import EmbeddedPlayer from "./EmbeddedPlayer.svelte";
  import SearchForm from "./SearchForm.svelte";
  import ToastContainer from "./ToastContainer.svelte";
  import TurnstileVerification from "./TurnstileVerification.svelte";
  import VideoResultItem from "./VideoResultItem.svelte";
  import {
    apiRequestWithRetry,
    ClientError,
    restoreVideoDetails,
    searchVideos as requestSearchVideos,
  } from "./api";
  import { createURL, parseData } from "./share";
  import {
    defaultPersistedState,
    loadPersistedState,
    type PlayerLoopType,
    savePersistedState,
    type SearchFormState,
  } from "./storage";
  import { notify } from "./notifications.svelte";
  import type { VideoContent, VideoPlayingData } from "./types";
  import togglePlayerButton from "./assets/toggle_player_button.svg";
  import searchButton from "./assets/search_button.svg";
  import shareButton from "./assets/share_button.svg";
  import githubLogo from "./assets/github_logo.svg";

  const initialState = loadPersistedState() ?? defaultPersistedState;

  let formExpand = $state(initialState.results.contentIds.length === 0);
  let playerEnabled = $state(
    initialState.player.enabled && initialState.results.selectedVideo != null,
  );
  let videos = $state<VideoContent[]>([]);
  let videoPlaying = $state<VideoPlayingData | null>(
    initialState.results.selectedVideo,
  );
  let shareOnlySearch = $state(false);
  let shareDropdown = $state<HTMLDivElement | null>(null);
  let loopType = $state<PlayerLoopType>(initialState.player.loopType);
  let searching = $state(false);
  let error = $state<string | null>(null);
  let initialSearchForm = $state<SearchFormState | null>(initialState.search);
  let searchForm = $state<SearchFormState>({ ...initialState.search });
  let showInitialLoadingSpinner = $state(false);
  let initialTurnstileRequest = $state<(() => void) | null>(null);
  let searchFormTurnstileRequest = $state<(() => void) | null>(null);
  let initialLoading = $state(true);

  $effect(() => {
    if (initialLoading) return;
    savePersistedState({
      version: 1,
      search: searchForm,
      results: {
        contentIds: videos.map((video) => video.contentId),
        selectedVideo: videoPlaying,
      },
      player: {
        enabled: playerEnabled,
        loopType,
      },
    });
  });

  onMount(async () => {
    const timerId = setTimeout(() => (showInitialLoadingSpinner = true), 500);
    try {
      if (!(await restoreSharedUrl())) {
        await restoreVideos(
          initialState.results.contentIds,
          initialState.results.selectedVideo,
        );
      }
    } finally {
      clearTimeout(timerId);
      initialLoading = false;
      showInitialLoadingSpinner = false;
      formExpand = videos.length === 0;
    }
  });

  async function restoreVideos(
    ids: string[],
    selectedVideo: VideoPlayingData | null,
  ) {
    videoPlaying = selectedVideo;
    playerEnabled = initialState.player.enabled && videoPlaying !== null;

    if (ids.length === 0) {
      return;
    }

    try {
      const result = await apiRequestWithRetry(
        () => restoreVideoDetails(ids),
        () => new Promise((resolve) => (initialTurnstileRequest = resolve)),
      );
      videos = ids.map((id) => result[id]).filter(Boolean);
    } catch (reason) {
      console.error(reason);
      notify("error", "前回の検索結果を復元できませんでした");
    } finally {
      initialTurnstileRequest = null;
    }
  }

  async function restoreSharedUrl(): Promise<boolean> {
    const data = new URLSearchParams(window.location.search).get("data");
    history.replaceState(null, "", window.location.pathname);
    if (data == null) return false;
    try {
      const result = await apiRequestWithRetry(
        () => parseData(data),
        () => new Promise((resolve) => (initialTurnstileRequest = resolve)),
      );
      initialSearchForm = result.search;
      videos = result.contents;
    } catch (reason) {
      console.error(reason);
      notify("error", "共有データの復元中にエラーが発生しました。");
      throw reason;
    } finally {
      initialTurnstileRequest = null;
    }
    return true;
  }

  async function searchVideos() {
    const search = { ...searchForm };
    searching = true;
    error = null;
    try {
      videos = await apiRequestWithRetry(
        () => requestSearchVideos(search),
        () => new Promise((resolve) => (searchFormTurnstileRequest = resolve)),
      );
      formExpand = false;
      error = null;
      searchFormTurnstileRequest = null;
    } catch (e) {
      if (e instanceof ClientError) {
        error = e.message;
      } else if (e instanceof Error) {
        console.error(e);
        notify("error", `検索中に予期せぬエラーが発生しました: ${e.message}`);
      } else {
        console.error(e);
        notify("error", `検索中に予期せぬエラーが発生しました: ${e}`);
      }
    } finally {
      searchFormTurnstileRequest = null;
      searching = false;
    }
  }

  function play(video: VideoContent) {
    if (videoPlaying?.contentId !== video.contentId) {
      videoPlaying = { contentId: video.contentId, tags: video.tags };
    }
    playerEnabled = true;
    scrollVideoIntoView(video.contentId);
  }

  function togglePlayer() {
    playerEnabled = !playerEnabled;
    if (playerEnabled && videoPlaying != null) {
      scrollVideoIntoView(videoPlaying.contentId);
    }
  }

  async function scrollVideoIntoView(contentId: string) {
    await tick();
    document.getElementById(`video-${contentId}`)?.scrollIntoView({
      behavior: "smooth",
      block: "center",
      inline: "center",
    });
  }

  function removeVideo(contentId: string) {
    videos = videos.filter((video) => video.contentId !== contentId);
  }

  function createUrlByCurrentState(
    currentVideos: VideoContent[] | null,
  ): Promise<string> {
    try {
      return createURL({
        search: searchForm,
        contentIds: currentVideos?.map((video) => video.contentId) ?? [],
      });
    } catch (e) {
      console.error(e);
      if (e instanceof Error) {
        notify("error", `共有URLの作成に失敗しました: ${e.message}`);
      } else {
        notify("error", `共有URLの作成に失敗しました: ${e}`);
      }
      throw e;
    }
  }

  function closeShareMenu() {
    const activeElement = document.activeElement;
    if (
      activeElement instanceof HTMLElement &&
      shareDropdown?.contains(activeElement)
    ) {
      activeElement.blur();
    }
  }

  async function copyShareUrl() {
    const url = await createUrlByCurrentState(shareOnlySearch ? [] : videos);
    try {
      await navigator.clipboard.writeText(url);
      closeShareMenu();
      notify("success", "リンクURLをコピーしました");
    } catch (reason) {
      console.error(reason);
      const message =
        reason instanceof Error
          ? reason.message
          : "共有URLの作成中にエラーが発生しました。";
      notify("error", `コピーに失敗しました: ${message}`);
    }
  }

  async function postToXShareUrl() {
    const url = await createUrlByCurrentState(shareOnlySearch ? [] : videos);
    const params = new URLSearchParams({
      text: `#NicoRandomPicker でランダムに動画を検索しま${shareOnlySearch ? "しょう" : "した"}！`,
      hashtags: "NicoRandomPickerShare",
      url,
    });
    window.open(
      `https://x.com/intent/tweet?${params}`,
      undefined,
      "popup,width=500,height=500",
    );
    closeShareMenu();
  }

  async function shareToMisskey() {
    const url = await createUrlByCurrentState(shareOnlySearch ? [] : videos);
    const params = new URLSearchParams({
      text: `#NicoRandomPicker でランダムに動画を検索しま${shareOnlySearch ? "しょう" : "した"}！`,
      url,
    });
    window.open(
      `https://misskey-hub.net/share?${params}`,
      undefined,
      "popup,width=700,height=700",
    );
    closeShareMenu();
  }

  function openInVocacolleApp() {
    if (videos.length === 0) {
      notify("error", "ボカコレアプリで開く動画がありません");
      return;
    }

    const params = new URLSearchParams({
      vid: videos.map((video) => video.contentId).join(","),
      title: createVocacollePlaylistTitle(),
      current: "1",
    });
    window.location.href = `nicobox://playlist?${params}`;
    closeShareMenu();
  }

  function createVocacollePlaylistTitle(): string {
    return searchForm.tag === ""
      ? "NicoRandomPicker"
      : `NicoRandomPicker: ${searchForm.tag}`;
  }
</script>

<div class="grid h-dvh grid-rows-[auto_minmax(0,1fr)] overflow-hidden">
  <div class="navbar bg-base-200 z-1">
    <div class="ps-4">
      <a class="text-lg font-bold" href="/">NicoRandomPicker</a>
    </div>
    <a
      class="btn btn-ghost ms-2"
      href="https://github.com/White-Green/nicorandompicker-v2"
      target="_blank"
      rel="noopener noreferrer"
      title="GitHubリポジトリを開く"
      aria-label="GitHubリポジトリを開く"
    >
      <img
        class="h-4 mix-blend-difference"
        src={githubLogo}
        alt="GitHub Logo"
      />
    </a>
    <div class="flex grow justify-end px-2">
      <div class="flex items-stretch">
        <div bind:this={shareDropdown} class="dropdown dropdown-end">
          <div
            tabindex="0"
            role="button"
            class="btn btn-ghost"
            title="検索結果を共有"
          >
            <img
              class="h-4 mix-blend-difference"
              alt="share"
              src={shareButton}
              height="auto"
              width="auto"
            />
          </div>
          <div
            class="dropdown-content card card-sm bg-base-100 z-1 w-64 shadow-md"
          >
            <div class="card-body">
              <label class="label cursor-pointer justify-between">
                <span class="label-text text-base-content"
                  >検索設定のみ共有</span
                >
                <input
                  class="checkbox checkbox-sm"
                  type="checkbox"
                  bind:checked={shareOnlySearch}
                />
              </label>
              <div class="card-actions flex-col">
                <button class="btn btn-primary w-full" onclick={copyShareUrl}
                  >リンクURLをコピー
                </button>
                <button class="btn btn-primary w-full" onclick={postToXShareUrl}
                  >リンクをXに投稿
                </button>
                <button class="btn btn-primary w-full" onclick={shareToMisskey}
                  >Misskeyで共有
                </button>
                <button
                  class="btn btn-primary w-full"
                  onclick={openInVocacolleApp}
                  >ボカコレアプリで開く
                </button>
              </div>
            </div>
          </div>
        </div>
        {#if videoPlaying != null}
          <button
            class="btn btn-ghost"
            type="button"
            title={playerEnabled ? "動画プレーヤーを閉じる": "再生中の動画に戻る"}
            aria-label={playerEnabled ? "動画プレーヤーを閉じる": "再生中の動画に戻る"}
            onclick={togglePlayer}
          >
            <img
              alt=""
              aria-hidden="true"
              class="h-4 mix-blend-difference"
              src={togglePlayerButton}
            />
          </button>
        {/if}
        <button
          class="btn btn-ghost"
          type="button"
          title={`検索メニューを${formExpand ? "閉じる" : "開く"}`}
          onclick={() => (formExpand = !formExpand)}
        >
          <img
            alt="search expand"
            class="h-4 mix-blend-difference"
            src={searchButton}
          />
        </button>
      </div>
    </div>
  </div>
  <div class="body h-full min-h-0 overflow-hidden @container-[size]">
    <main
      class={`main_layout grid h-full min-h-0 overflow-hidden grid-cols-1 grid-rows-[minmax(0,1fr)_auto] xl:grid-rows-1 ${playerEnabled ? "xl:grid-cols-[minmax(0,min(calc(100cqw-360px),max(0px,calc((100cqh-10rem)*16/9))))_minmax(360px,1fr)]" : ""}`}
    >
      <div
        class={`${playerEnabled ? "" : "hidden"} row-start-2 min-h-0 xl:col-start-1 xl:row-start-1`}
      >
        <EmbeddedPlayer
          bind:selectedVideo={videoPlaying}
          bind:loopType
          {videos}
          enabled={playerEnabled}
          onClose={() => (playerEnabled = false)}
        />
      </div>
      <div
        class={`row-start-1 min-h-0 overflow-y-auto w-full ${playerEnabled ? "xl:col-start-2" : "xl:col-start-1"}`}
      >
        <div
          class={`m-4 grid grid-cols-[repeat(auto-fit,minmax(320px,1fr))] gap-4 ${playerEnabled ? "xl:ml-0" : ""}`}
        >
          {#each videos as video (video.contentId)}
            <VideoResultItem
              {video}
              selected={videoPlaying?.contentId === video.contentId}
              onPlay={play}
              onDelete={removeVideo}
            />
          {/each}
          {#each new Array(10)}
            <div></div>
          {/each}
        </div>
      </div>
    </main>

    <SearchForm
      bind:value={searchForm}
      initialValue={initialSearchForm}
      expanded={formExpand}
      {searching}
      {error}
      onSearch={searchVideos}
      turnstileVerifyRequest={searchFormTurnstileRequest}
    />
    {#if showInitialLoadingSpinner}
      {#if initialTurnstileRequest == null}
        <div class="modal modal-open">
          <div class="modal-box text-center">
            <span class="loading loading-spinner loading-xl text-primary"
            ></span>
            <h2 class="mt-4 text-lg font-bold">検索結果を復元しています</h2>
          </div>
        </div>
      {:else}
        <dialog class="modal modal-open">
          <div class="modal-box">
            <h2 class="text-lg font-bold">bot対策にご協力ください</h2>
            <p class="py-4">
              初回ロードの復元リクエストが制限されました。検証後に自動で再開します。
            </p>
            <div class="flex justify-center">
              <TurnstileVerification onVerified={initialTurnstileRequest} />
            </div>
          </div>
        </dialog>
      {/if}
    {/if}
    <ToastContainer />
  </div>
</div>
