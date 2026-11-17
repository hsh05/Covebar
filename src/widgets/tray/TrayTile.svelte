<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { scale } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import Icon from "../../lib/Icon.svelte";
  import { tray } from "../../lib/live.svelte";
  import { iconUrl } from "../../lib/icons";
  import type { TrayIcon } from "../../lib/types";

  const live = tray();
  const st = $derived(live.v);

  // Program icons for tray items Windows has no snapshot of.
  let fallback = $state<Record<string, string | null>>({});
  $effect(() => {
    for (const i of st.icons) {
      if (i.image || !i.iconKey || i.iconKey in fallback) continue;
      const key = i.iconKey;
      fallback[key] = null;
      iconUrl(key, 16).then((u) => (fallback[key] = u));
    }
  });
  const src = (i: TrayIcon) => i.image ?? (i.iconKey ? fallback[i.iconKey] : null);

  let root: HTMLDivElement;
  function click(id: string, button: "left" | "right" | "overflow") {
    const r = root.getBoundingClientRect();
    invoke("tray_click", { id, button, anchorX: r.left, anchorWidth: r.width });
  }
  function right(e: MouseEvent, id: string) {
    e.preventDefault();
    e.stopPropagation();
    click(id, "right");
  }
</script>

<div class="tray" data-nodrag bind:this={root}>
  {#if st.hasOverflow}
    <button class="tile glyph chevron" onclick={() => click("", "overflow")} title="Show hidden icons">
      <Icon name="chevronUp" size={16} />
    </button>
  {/if}
  {#each st.icons as i (i.id)}
    {@const url = src(i)}
    <button
      class="tile item"
      onclick={() => click(i.id, "left")}
      oncontextmenu={(e) => right(e, i.id)}
      title={i.tooltip}
      animate:flip={{ duration: 260, easing: cubicOut }}
      transition:scale={{ duration: 200, start: 0.6, easing: cubicOut }}
    >
      {#if url}
        <img src={url} alt="" draggable="false" />
      {:else}
        <span class="dot">{i.tooltip.trim().charAt(0).toUpperCase() || "•"}</span>
      {/if}
    </button>
  {/each}
  {#if st.unavailable}
    <span class="tile glyph muted-text" title="Covebar can't read the Windows tray on this version of Windows">
      <Icon name="tray" size="var(--glyph)" />
    </span>
  {/if}
</div>

<style>
  .tray {
    display: flex;
    align-items: center;
    gap: 1px;
  }
  .item {
    width: 30px;
    padding: 0;
  }
  :global(:root[data-size="large"]) .item {
    width: 36px;
  }
  .chevron {
    min-width: 26px;
    padding: 0 5px;
    color: var(--text-dim);
  }
  img {
    width: 16px;
    height: 16px;
    image-rendering: auto;
  }
  :global(:root[data-size="large"]) img {
    width: 20px;
    height: 20px;
  }
  .dot {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    background: var(--raised);
    font-size: 10px;
    font-weight: 700;
    color: var(--text-dim);
  }
</style>
