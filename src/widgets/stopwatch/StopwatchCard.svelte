<script lang="ts">
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Icon from "../../lib/Icon.svelte";
  import { ticker, timers, timersAction } from "../../lib/live.svelte";
  import { duration } from "../../lib/format";

  const live = timers();
  const sw = $derived(live.v.stopwatch);
  const running = $derived(sw.startedAt != null);
  const clock = ticker(31, () => running);
  const elapsed = $derived(sw.elapsedMs + (sw.startedAt != null ? clock.now - sw.startedAt : 0));

  // Newest lap first, each with its own split time; mark the fastest and slowest.
  const laps = $derived.by(() => {
    const rows = sw.laps.map((total, i) => ({ n: i + 1, total, split: total - (i ? sw.laps[i - 1] : 0) }));
    let fast = -1;
    let slow = -1;
    if (rows.length >= 3) {
      const splits = rows.map((r) => r.split);
      fast = splits.indexOf(Math.min(...splits));
      slow = splits.indexOf(Math.max(...splits));
    }
    return rows.map((r, i) => ({ ...r, fast: i === fast, slow: i === slow })).reverse();
  });
  const current = $derived(elapsed - (sw.laps.at(-1) ?? 0));
</script>

<div class="card">
  <div class="card-title">
    <Icon name="stopwatch" size={16} />
    <span class="grow">Stopwatch</span>
  </div>

  <div class="readout" class:paused={!running && sw.elapsedMs > 0}>
    {duration(elapsed, { hundredths: true })}
  </div>

  <div class="actions">
    {#if running}
      <button class="btn" onclick={() => timersAction("swLap")}><Icon name="flag" size={14} /> Lap</button>
      <button class="btn primary" onclick={() => timersAction("swPause")}><Icon name="pause" size={14} /> Pause</button>
    {:else}
      <button class="btn" disabled={sw.elapsedMs === 0} onclick={() => timersAction("swReset")}>
        <Icon name="reset" size={14} /> Reset
      </button>
      <button class="btn primary" onclick={() => timersAction("swStart")}>
        <Icon name="play" size={14} />
        {sw.elapsedMs > 0 ? "Resume" : "Start"}
      </button>
    {/if}
  </div>

  <div class="laps">
    {#if laps.length || running}
      <div class="lap head"><span>Lap</span><span>Split</span><span>Total</span></div>
    {/if}
    <div class="list">
      {#if laps.length && running}
        <div class="lap live">
          <span>Lap {laps.length + 1}</span><span>{duration(current, { hundredths: true })}</span><span
            >{duration(elapsed, { hundredths: true })}</span
          >
        </div>
      {/if}
      {#each laps as lap (lap.n)}
        <div class="lap" class:fast={lap.fast} class:slow={lap.slow} in:slide={{ duration: 200, easing: cubicOut }}>
          <span>Lap {lap.n}</span>
          <span>{duration(lap.split, { hundredths: true })}</span>
          <span>{duration(lap.total, { hundredths: true })}</span>
        </div>
      {/each}
      {#if !laps.length}
        <p class="empty">{running ? "Press Lap to record a split" : "Laps you record are kept until you reset"}</p>
      {/if}
    </div>
  </div>
</div>

<style>
  .readout {
    text-align: center;
    font: 600 40px/1.1 var(--font-display);
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.5px;
    padding: 6px 0 2px;
    transition: opacity var(--t-med) var(--ease-out);
  }
  .readout.paused {
    opacity: 0.6;
  }
  .actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .laps {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--line);
    padding-top: 6px;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    scrollbar-width: thin;
  }
  .lap {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    padding: 5px 4px;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .lap span:not(:first-child) {
    text-align: right;
  }
  .lap + .lap {
    border-top: 1px solid var(--line);
  }
  .head {
    color: var(--text-faint);
    font-size: 11px;
    padding-top: 0;
  }
  .live {
    color: var(--text-dim);
  }
  .fast {
    color: var(--accent);
  }
  .slow {
    color: var(--danger);
  }
  .empty {
    margin: 14px 0 0;
    text-align: center;
    font-size: 12px;
    color: var(--text-faint);
  }
</style>
