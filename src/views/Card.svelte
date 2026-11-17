<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { WIDGETS } from "../lib/widgets";

  let widget = $state<string | null>(null);
  // Bumped on every open so the card's content starts fresh each time.
  let openCount = $state(0);

  listen<{ widget: string }>("card-open", (e) => {
    widget = e.payload.widget;
    openCount++;
  });
  getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (!focused) invoke("close_card");
  });
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") invoke("close_card");
  });

  const def = $derived(widget ? WIDGETS[widget] : null);
</script>

<div class="card">
  {#if def?.card}
    {#key openCount}
      <def.card />
    {/key}
  {/if}
</div>

<style>
  .card {
    height: 100vh;
    background: var(--card-tint);
    overflow: hidden;
  }
</style>
