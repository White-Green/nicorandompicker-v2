<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { dismissNotification, notifications } from "./notifications.svelte";
  import type { Notification } from "./notifications.svelte";

  function alertClass(notification: Notification): string {
    if (notification.type === "success") return "alert alert-success";
    if (notification.type === "error") return "alert alert-error";
    return "alert alert-info";
  }
</script>

<div
  class="toast toast-end toast-top toast_container mt-16"
  aria-live="polite"
  aria-atomic="false"
>
  {#each notifications as notification (notification.id)}
    <section
      class={`${alertClass(notification)} notification_toast`}
      role={notification.type === "error" ? "alert" : "status"}
      in:fly={{ x: 16, duration: 120 }}
      out:fade={{ duration: 240 }}
    >
      <span>{notification.message}</span>
      <button
        type="button"
        class="notification_toast_close"
        aria-label="通知を閉じる"
        onclick={() => dismissNotification(notification.id)}
      >
        ×
      </button>
    </section>
  {/each}
</div>
