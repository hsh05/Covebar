<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { tick } from "svelte";
  import { iconUrl } from "../lib/icons";
  import { PREVIEW } from "../lib/popups";
  import { ENTER_MS } from "../lib/motion";
  import type { PreviewData } from "../lib/types";

  let data = $state<PreviewData | null>(null);
  let icon = $state<string | null>(null);
  let root = $state<HTMLElement>();
  let shown = $state(0);
  let out = $state(false);
  let thumbTimer: ReturnType<typeof setTimeout> | undefined;

  listen<PreviewData>("preview-data", async (e) => {
    data = e.payload;
    out = false;
    shown++;
    iconUrl(e.payload.icon, 16).then((u) => (icon = u));
    // Live thumbnails can't animate, so attach them once the panel has settled.
    clearTimeout(thumbTimer);
    await tick();
    thumbTimer = setTimeout(registerThumbs, ENTER_MS + 10);
  });
  listen("preview-out", () => {
    clearTimeout(thumbTimer);
    out = true;
  });

  // Tell the backend where each live thumbnail goes (physical pixels in this window).
  function registerThumbs() {
    if (!data || !root) return;
    const k = window.devicePixelRatio || 1;
    const items = [...root.querySelectorAll<HTMLElement>("[data-thumb]")].map((el) => {
      const r = el.getBoundingClientRect();
      return { hwnd: Number(el.dataset.thumb), x: r.left * k, y: r.top * k, w: r.width * k, h: r.height * k };
    });
    invoke("set_thumbnails", { items });
  }

  async function closeWin(hwnd: number, e: MouseEvent) {
    e.stopPropagation();
    invoke("close_window", { hwnd });
    if (!data) return;
    data = { ...data, windows: data.windows.filter((w) => w.hwnd !== hwnd) };
    if (!data.windows.length) {
      invoke("hide_preview");
      return;
    }
    await tick();
    registerThumbs();
  }
</script>

{#key shown}
<div
  class="surface preview"
  class:out
  bind:this={root}
  style:--tw="{PREVIEW.thumbW}px"
  style:--th="{PREVIEW.thumbH}px"
  style:--hd="{PREVIEW.header}px"
  style:--pad="{PREVIEW.pad}px"
  style:--gap="{PREVIEW.gap}px"
  style:--cols={Math.min(data?.windows.length ?? 1, PREVIEW.perRow)}
  onpointerenter={() => invoke("preview_hold")}
  onpointerleave={() => invoke("preview_release")}
  role="group"
  aria-label="Window previews"
>
  {#if data}
    {#each data.windows as w (w.hwnd)}
      <button class="item" onclick={() => invoke("activate_window", { hwnd: w.hwnd })}>
        <div class="head">
          {#if icon}<img src={icon} alt="" width="16" height="16" />{/if}
          <span class="title">{w.title}</span>
          <span
            class="close"
            role="button"
            tabindex="-1"
            aria-label="Close window"
            onclick={(e) => closeWin(w.hwnd, e)}
            onkeydown={() => {}}
          >
            <svg viewBox="0 0 12 12"><path d="M3 3l6 6M9 3l-6 6" /></svg>
          </span>
        </div>
        <div class="thumb" data-thumb={w.hwnd}></div>
      </button>
    {/each}
  {/if}
</div>
{/key}

<style>
  .preview {
    padding: var(--pad);
    display: grid;
    grid-template-columns: repeat(var(--cols), var(--tw));
    grid-auto-rows: calc(var(--hd) + var(--th));
    gap: var(--gap);
    align-content: start;
  }
  .item {
    display: flex;
    flex-direction: column;
    border-radius: 8px;
    text-align: left;
    overflow: hidden;
  }
  .item {
    transition: background-color var(--t-fast) var(--ease-out);
  }
  .item:hover {
    background: var(--hover);
  }
  .head {
    height: var(--hd);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 4px 0 8px;
  }
  .title {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 12px;
  }
  .close {
    width: 22px;
    height: 22px;
    border-radius: 5px;
    display: grid;
    place-items: center;
    opacity: 0;
  }
  .item:hover .close {
    opacity: 1;
  }
  .close:hover {
    background: var(--danger);
    color: white;
  }
  .close svg {
    width: 10px;
    height: 10px;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
  }
  .thumb {
    height: calc(var(--th) - 6px);
    margin: 0 6px 6px;
  }
</style>
