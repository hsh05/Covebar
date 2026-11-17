<script lang="ts">
  import Icon from "../../lib/Icon.svelte";
  import { ticker, timers } from "../../lib/live.svelte";
  import { openCard } from "../../lib/popups";
  import { duration } from "../../lib/format";

  const live = timers();
  const sw = $derived(live.v.stopwatch);
  const running = $derived(sw.startedAt != null);
  const clock = ticker(200, () => running);
  const elapsed = $derived(sw.elapsedMs + (sw.startedAt != null ? clock.now - sw.startedAt : 0));
  const active = $derived(running || sw.elapsedMs > 0);

  let el: HTMLButtonElement;
</script>

<button
  class="tile glyph"
  class:running
  bind:this={el}
  onclick={() => openCard("stopwatch", el, 300, 392)}
  data-nodrag
  title={active ? `Stopwatch: ${duration(elapsed)}` : "Stopwatch"}
>
  <span class="glyph-wrap"><Icon name="stopwatch" size="var(--glyph)" /></span>
  {#if active}
    <span class="value" class:paused={!running}>{duration(elapsed)}</span>
  {/if}
</button>

<style>
  .glyph-wrap {
    display: grid;
  }
  .running .glyph-wrap {
    color: var(--accent);
  }
  .value.paused {
    opacity: 0.55;
  }
</style>
