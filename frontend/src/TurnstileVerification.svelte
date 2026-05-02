<script module lang="ts">
  type TurnstileApi = NonNullable<Window["turnstile"]>;
  const TURNSTILE_ONLOAD_CALLBACK = "__nrpTurnstileOnLoad";
  let turnstileReady: Promise<TurnstileApi> | null = null;

  function loadTurnstile(): Promise<TurnstileApi> {
    if (window.turnstile !== undefined)
      return Promise.resolve(window.turnstile);
    if (turnstileReady) return turnstileReady;

    turnstileReady = new Promise<TurnstileApi>((resolve, reject) => {
      const cleanup = () => {
        window[TURNSTILE_ONLOAD_CALLBACK] = undefined;
      };

      window[TURNSTILE_ONLOAD_CALLBACK] = () => {
        if (window.turnstile === undefined) {
          cleanup();
          turnstileReady = null;
          reject(new Error("Turnstile API was not initialized"));
          return;
        }

        const api = window.turnstile;
        cleanup();
        resolve(api);
      };

      const script = document.createElement("script");
      script.src = `https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit&onload=${TURNSTILE_ONLOAD_CALLBACK}`;
      script.onerror = () => {
        cleanup();
        turnstileReady = null;
        reject(new Error("failed to load Turnstile"));
      };
      document.head.append(script);
    });

    return turnstileReady;
  }
</script>

<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { verifyTurnstile } from "./api";
  import { notify } from "./notifications.svelte";
  import { clearTurnstileRecommendation } from "./turnstileState.svelte";

  interface Props {
    onVerified: null | (() => void | Promise<void>);
  }

  let { onVerified }: Props = $props();

  const siteKey = import.meta.env.VITE_TURNSTILE_SITE_KEY;
  let container = $state<HTMLDivElement | null>(null);
  let widgetId: string | null = null;
  let turnstileApi: TurnstileApi | null = null;
  let mounted = true;

  onMount(async () => {
    turnstileApi = await loadTurnstile();
    while (true) {
      try {
        const token = await new Promise<string>((resolve, reject) => {
          if (turnstileApi == null) throw new Error("UNREACHABLE");
          if (!mounted || container == null) return;
          if (widgetId != null) {
            turnstileApi.reset(container);
          } else {
            widgetId =
              turnstileApi.render(container, {
                sitekey: siteKey,
                callback: resolve,
                "error-callback": reject,
                "refresh-expired": "auto",
              }) ?? null;
          }
          if (widgetId == null) reject();
        });
        const verifyResponse = await verifyTurnstile(token);
        if (verifyResponse == undefined) {
          clearTurnstileRecommendation();
          onVerified?.();
          return;
        }
        console.error(
          "Unexpected response from verifyTurnstile",
          verifyResponse,
        );
        if (verifyResponse.status === 429) {
          const retryAfter = Number.parseInt(
            verifyResponse.headers.get("Retry-After") ?? "",
          );
          await new Promise((resolve) =>
            setTimeout(
              resolve,
              Number.isFinite(retryAfter) ? retryAfter * 1000 : 10_000,
            ),
          );
        } else {
          notify("error", "Turnstileの検証に失敗しました");
        }
      } catch (reason) {
        console.error(reason);
        notify("error", "Turnstileの検証に失敗しました");
      } finally {
        await new Promise((resolve) => setTimeout(resolve, 0));
        if (widgetId != null) {
          turnstileApi.remove(widgetId);
          widgetId = null;
        }
      }
    }
  });

  onDestroy(() => {
    mounted = false;
    if (widgetId !== null) {
      turnstileApi?.remove(widgetId);
    }
  });
</script>

<div bind:this={container}></div>
