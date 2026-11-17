<script lang="ts">
  import { scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Icon from "../../lib/Icon.svelte";
  import { recycle } from "../../lib/live.svelte";
  import { openCard } from "../../lib/popups";
  import { bytes } from "../../lib/format";

  const live = recycle();
  const bin = $derived(live.v);
  let el: HTMLButtonElement;
</script>

<button
  class="tile glyph bin"
  bind:this={el}
  onclick={() => openCard("recycle", el, 280, 196)}
  data-nodrag
  title={bin.items ? `Recycle Bin: ${bin.items} item${bin.items === 1 ? "" : "s"}, ${bytes(bin.bytes)}` : "Recycle Bin is empty"}
>
  <span class="wrap">
    <Icon name={bin.items ? "binFull" : "bin"} size="var(--glyph)" />
    {#if bin.items}
      <span class="badge" transition:scale={{ duration: 200, easing: cubicOut }}>
        {bin.items > 99 ? "99+" : bin.items}
      </span>
    {/if}
  </span>
</button>

<style>
  .wrap {
    position: relative;
    display: grid;
  }
  .badge {
    position: absolute;
    top: -5px;
    right: -9px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 10px;
    font-weight: 700;
    line-height: 16px;
    text-align: center;
  }
</style>
