<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { WIDGETS } from "../lib/widgets";
  import { EXIT_MS } from "../lib/motion";

  let widget = $state<string | null>(null);
  // Bumped on every open so the card's content (and its entrance animation) starts fresh.
  let openCount = $state(0);
  let closing = $state(false);

  listen<{ widget: string }>("card-open", (e) => {
    widget = e.payload.widget;
    closing = false;
    openCount++;
  });

  // Play the exit animation, then ask the backend to hide this card (if it's still this one).
  function close() {
    if (closing || !widget) return;
    closing = true;
    const w = widget;
    setTimeout(() => invoke("close_card", { widget: w }), EXIT_MS);
  }

  getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (!focused) close();
  });
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") close();
  });

  const def = $derived(widget ? WIDGETS[widget] : null);

  // Cards marked `fit` size the window to their content (and follow it as it changes).
  function fit(el: HTMLElement, id: string) {
    let last = 0;
    const ro = new ResizeObserver(() => {
      const h = Math.ceil(el.offsetHeight) + 2; // + the surface's border
      if (Math.abs(h - last) > 1) {
        last = h;
        invoke("resize_card", { widget: id, height: h });
      }
    });
    ro.observe(el);
    return { destroy: () => ro.disconnect() };
  }
</script>

{#key openCount}
  {#if def?.card}
    <div class="surface" class:out={closing}>
      {#if def.fit}
        <div class="fit" use:fit={widget!}><def.card /></div>
      {:else}
        <def.card />
      {/if}
    </div>
  {/if}
{/key}

<style>
  .fit :global(.card) {
    height: auto;
  }
</style>
