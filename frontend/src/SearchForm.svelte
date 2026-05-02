<script lang="ts">
  import TurnstileVerification from "./TurnstileVerification.svelte";
  import type { SearchFormState } from "./storage";
  import { turnstileRecommended } from "./turnstileState.svelte";

  interface Props {
    value: SearchFormState;
    initialValue: SearchFormState | null;
    expanded: boolean;
    searching: boolean;
    error: string | null;
    onSearch: () => void;
    turnstileVerifyRequest: null | (() => void);
  }

  let {
    value = $bindable(),
    initialValue,
    expanded,
    searching,
    error,
    onSearch,
    turnstileVerifyRequest,
  }: Props = $props();

  let formElement = $state<HTMLFormElement | null>(null);
  let tag = $state("");
  let uploadedSince = $state("");
  let uploadedUntil = $state("");
  let viewMin = $state("");
  let viewMax = $state("");
  let resultCount = $state("10");

  $effect(() => {
    if (initialValue == null) return;
    tag = initialValue.tag;
    uploadedSince = initialValue.uploadedSince ?? "";
    uploadedUntil = initialValue.uploadedUntil ?? "";
    viewMin = numberStringFromNullable(initialValue.viewMin);
    viewMax = numberStringFromNullable(initialValue.viewMax);
    resultCount = initialValue.resultCount.toString();
  });

  $effect(() => {
    value = createSearchFormState();
  });

  function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    value = createSearchFormState();
    if (!formElement?.reportValidity()) return;
    onSearch();
  }

  function createSearchFormState(): SearchFormState {
    return {
      tag,
      uploadedSince: stringOrNull(uploadedSince),
      uploadedUntil: stringOrNull(uploadedUntil),
      viewMin: numberOrNull(viewMin),
      viewMax: numberOrNull(viewMax),
      resultCount: numberOrNull(resultCount) ?? 10,
    };
  }

  function numberOrNull(value: string | null | undefined): number | null {
    if (value === "" || value == null) return null;
    const parsed = typeof value === "number" ? value : Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }

  function stringOrNull(value: string): string | null {
    return value === "" ? null : value;
  }

  function numberStringFromNullable(value: number | null): string {
    return value === null ? "" : value.toString();
  }
</script>

<section
  class={`fixed top-16 right-0 left-0 mx-auto max-w-285 overflow-y-auto lg:rounded-b-2xl lg:border-x-2 border-b-2 border-neutral-content bg-base-100 text-center transition-transform duration-500 ${expanded ? "translate-y-0" : "-translate-y-full"}`}
>
  <form bind:this={formElement} class="m-5" onsubmit={handleSubmit}>
    <div class="my-3">
      <label class="mb-1 block" for="tag_name_input">タグ</label>
      <input
        bind:value={tag}
        type="text"
        class="input w-full"
        id="tag_name_input"
        name="tag_name"
        placeholder="タグを空白区切りで入力してください"
        required
      />
    </div>
    <div class="my-3">
      <label class="mb-1 block" for="start_time_input"
        >アップロード日時フィルタ</label
      >
      <div class="grid grid-cols-2 gap-3 6xl:gap-6" id="start_time_input">
        <div class="my-3">
          <label class="mb-1 block" for="start_time_from_input">ここから</label>
          <input
            bind:value={uploadedSince}
            type="datetime-local"
            class="input w-full"
            id="start_time_from_input"
            name="start_time_from"
          />
        </div>
        <div class="my-3">
          <label class="mb-1 block" for="start_time_to_input">ここまで</label>
          <input
            bind:value={uploadedUntil}
            type="datetime-local"
            class="input w-full"
            id="start_time_to_input"
            name="start_time_to"
          />
        </div>
      </div>
    </div>
    <div class="my-3">
      <label class="mb-1 block" for="view_counter_input">再生数フィルタ</label>
      <div class="grid grid-cols-2 gap-3 xl:gap-6" id="view_counter_input">
        <div class="my-3">
          <label class="mb-1 block" for="view_counter_min_input">これ以上</label
          >
          <input
            bind:value={viewMin}
            type="number"
            class="input w-full"
            id="view_counter_min_input"
            name="view_counter_min"
            min="0"
            max="100000000"
            placeholder="0以上"
          />
        </div>
        <div class="my-3">
          <label class="mb-1 block" for="view_counter_max_input">これ以下</label
          >
          <input
            bind:value={viewMax}
            type="number"
            class="input w-full"
            id="view_counter_max_input"
            name="view_counter_max"
            min="0"
            max="100000000"
            placeholder="100,000,000以下"
          />
        </div>
      </div>
    </div>
    <div class="my-3">
      <label class="mb-1 block" for="video_count_input">検索件数上限</label>
      <input
        bind:value={resultCount}
        type="number"
        class="input w-full"
        id="video_count_input"
        name="video_count"
        min="1"
        max="100"
        placeholder="0~100"
        required
      />
    </div>
    <div
      class="my-3 grid grid-cols-[1fr_auto_1fr] items-center gap-4 max-md:grid-cols-1"
    >
      <button
        id="submit_button"
        class="btn btn-primary col-start-2 row-start-1 w-auto min-w-16 justify-self-center max-md:col-auto"
        disabled={searching || turnstileRecommended()}
        title="本日朝5時までに投稿された動画が対象です。"
      >
        {searching ? "検索中" : "探す"}
      </button>
      {#if turnstileRecommended() || turnstileVerifyRequest != null}
        <div
          class="col-start-3 row-start-1 justify-self-start max-md:col-auto max-md:row-start-2 max-md:justify-self-center"
        >
          <TurnstileVerification onVerified={turnstileVerifyRequest} />
        </div>
      {/if}
      {#if error}
        <div
          class="alert alert-error col-start-1 row-start-1 m-0 justify-self-stretch text-left max-md:col-auto max-md:row-start-3 max-md:text-center"
        >
          エラー: {error}
        </div>
      {:else if turnstileVerifyRequest != null}
        <div
          class="alert alert-warning col-start-1 row-start-1 m-0 justify-self-stretch text-left max-md:col-auto max-md:row-start-3 max-md:text-center"
        >
          bot対策にご協力ください
        </div>
      {/if}
    </div>
  </form>
</section>
