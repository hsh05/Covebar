<script lang="ts">
  import { onDestroy } from "svelte";
  import { store } from "../../lib/settings.svelte";
  import { longDate, timeParts } from "../../lib/format";

  let now = $state(new Date());
  const timer = setInterval(() => (now = new Date()), 1000);
  onDestroy(() => clearInterval(timer));

  const opened = new Date();
  let view = $state({ y: opened.getFullYear(), m: opened.getMonth() });
  const t = $derived(timeParts(now, store.s?.clock24h ?? true));
  const monday = $derived(store.s?.weekStartsMonday ?? true);

  const heads = $derived(
    monday ? ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] : ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"],
  );

  const days = $derived.by(() => {
    const first = new Date(view.y, view.m, 1);
    const offset = (first.getDay() - (monday ? 1 : 0) + 7) % 7;
    const start = new Date(view.y, view.m, 1 - offset);
    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
      return {
        d,
        other: d.getMonth() !== view.m,
        today: d.toDateString() === now.toDateString(),
      };
    });
  });

  const title = $derived(
    new Date(view.y, view.m, 1).toLocaleDateString("en-GB", { month: "long", year: "numeric" }),
  );

  function shift(n: number) {
    const d = new Date(view.y, view.m + n, 1);
    view = { y: d.getFullYear(), m: d.getMonth() };
  }
  function today() {
    view = { y: now.getFullYear(), m: now.getMonth() };
  }
</script>

<div class="cal">
  <header class="now">
    <div class="big">
      {t.hm}<span class="sec">:{t.s}</span>{#if t.suffix}<span class="ampm">{t.suffix}</span>{/if}
    </div>
    <div class="long">{longDate(now)}</div>
  </header>

  <div class="nav">
    <button class="month" onclick={today} title="Back to today">{title}</button>
    <div class="arrows">
      <button onclick={() => shift(-1)} aria-label="Previous month">
        <svg viewBox="0 0 16 16"><path d="M10 3 5 8l5 5" /></svg>
      </button>
      <button onclick={() => shift(1)} aria-label="Next month">
        <svg viewBox="0 0 16 16"><path d="m6 3 5 5-5 5" /></svg>
      </button>
    </div>
  </div>

  <div class="grid">
    {#each heads as h}<span class="head">{h}</span>{/each}
    {#each days as day}
      <span class="day" class:other={day.other} class:today={day.today}>{day.d.getDate()}</span>
    {/each}
  </div>
</div>

<style>
  .cal {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 16px 16px 14px;
  }
  .now {
    padding-bottom: 12px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 10px;
  }
  .big {
    font: 600 34px/1.1 var(--font-display);
    font-variant-numeric: tabular-nums;
  }
  .sec {
    color: var(--text-dim);
    font-weight: 500;
  }
  .ampm {
    font-size: 15px;
    margin-left: 6px;
    color: var(--text-dim);
  }
  .long {
    color: var(--text-dim);
    margin-top: 2px;
  }
  .nav {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  .month {
    font-weight: 600;
    padding: 4px 8px;
    margin-left: -8px;
    border-radius: 6px;
  }
  .month:hover,
  .arrows button:hover {
    background: var(--hover);
  }
  .arrows {
    display: flex;
    gap: 2px;
  }
  .arrows button {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    display: grid;
    place-items: center;
  }
  svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
  .head {
    font-size: 11px;
    color: var(--text-faint);
    padding: 4px 0;
  }
  .day {
    height: 34px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    width: 34px;
    justify-self: center;
  }
  .other {
    color: var(--text-faint);
  }
  .today {
    background: var(--accent);
    color: var(--accent-text);
    font-weight: 600;
  }
</style>
