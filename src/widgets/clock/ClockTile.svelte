<script lang="ts">
  import { onDestroy } from "svelte";
  import { store } from "../../lib/settings.svelte";
  import { openCard } from "../../lib/popups";
  import { shortDate, timeParts } from "../../lib/format";
  import { WIDGETS } from "../../lib/widgets";

  let now = $state(new Date());
  let timer: ReturnType<typeof setTimeout>;
  function tick() {
    now = new Date();
    timer = setTimeout(tick, 1000 - (Date.now() % 1000) + 5);
  }
  tick();
  onDestroy(() => clearTimeout(timer));

  const t = $derived(timeParts(now, store.s?.clock24h ?? true));
  let el: HTMLButtonElement;

  function open() {
    const size = WIDGETS.clock.cardSize!;
    openCard("clock", el, size.width, size.height);
  }
</script>

<button class="tile clock" bind:this={el} onclick={open} data-nodrag title="Calendar">
  <span class="time">
    {t.hm}{#if store.s?.clockSeconds}<span class="sec">:{t.s}</span>{/if}{#if t.suffix}<span class="ampm">{t.suffix}</span>{/if}
  </span>
  <span class="date">{shortDate(now)}</span>
</button>

<style>
  .clock {
    flex-direction: column;
    align-items: flex-end;
    justify-content: center;
    padding: 0 12px;
    gap: 1px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .time {
    font: 600 var(--clock-time) / 1.1 var(--font-display);
    letter-spacing: 0.2px;
  }
  .sec {
    color: var(--text-dim);
    font-weight: 500;
  }
  .ampm {
    margin-left: 4px;
    font-size: 0.68em;
    font-weight: 600;
    color: var(--text-dim);
  }
  .date {
    font-size: var(--clock-date);
    color: var(--text-dim);
  }
</style>
