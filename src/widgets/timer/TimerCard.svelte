<script lang="ts">
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Icon from "../../lib/Icon.svelte";
  import { timers } from "../../lib/live.svelte";
  import { timerPhase } from "./timer";
  import TimerPane from "./TimerPane.svelte";
  import StopwatchPane from "./StopwatchPane.svelte";

  type Tab = "timer" | "stopwatch";
  const KEY = "covebar.timerTab";
  const live = timers();

  // Open on whichever one is in use; when both or neither are, on the last tab used.
  function firstTab(): Tab {
    const timerOn = timerPhase(live.v.timer) !== "idle";
    const sw = live.v.stopwatch;
    const swOn = sw.startedAt != null || sw.elapsedMs > 0;
    if (timerOn !== swOn) return timerOn ? "timer" : "stopwatch";
    try {
      if (localStorage.getItem(KEY) === "stopwatch") return "stopwatch";
    } catch {}
    return "timer";
  }

  let tab = $state<Tab>(firstTab());
  // Slide the incoming pane in from the side of the tab that was picked.
  let dir = $state(1);
  function choose(t: Tab) {
    if (t === tab) return;
    dir = t === "stopwatch" ? 1 : -1;
    tab = t;
    try {
      localStorage.setItem(KEY, t);
    } catch {}
  }

  const swRunning = $derived(live.v.stopwatch.startedAt != null);
  const timerPhaseNow = $derived(timerPhase(live.v.timer));
</script>

<div class="card">
  <div class="tabs" role="tablist" style:--i={tab === "timer" ? 0 : 1}>
    <span class="thumb"></span>
    <button role="tab" aria-selected={tab === "timer"} class:on={tab === "timer"} onclick={() => choose("timer")}>
      <Icon name="hourglass" size={14} /> Timer
      {#if timerPhaseNow === "running" || timerPhaseNow === "ringing"}<span class="live" class:alert={timerPhaseNow === "ringing"}></span>{/if}
    </button>
    <button role="tab" aria-selected={tab === "stopwatch"} class:on={tab === "stopwatch"} onclick={() => choose("stopwatch")}>
      <Icon name="stopwatch" size={14} /> Stopwatch
      {#if swRunning}<span class="live"></span>{/if}
    </button>
  </div>

  <div class="stage">
    {#key tab}
      <div
        class="slot"
        in:fly={{ x: 28 * dir, duration: 240, easing: cubicOut }}
        out:fly={{ x: -28 * dir, duration: 160, easing: cubicOut }}
      >
        {#if tab === "timer"}<TimerPane />{:else}<StopwatchPane />{/if}
      </div>
    {/key}
  </div>
</div>

<style>
  .tabs {
    position: relative;
    display: grid;
    grid-template-columns: 1fr 1fr;
    padding: 3px;
    border-radius: 10px;
    background: var(--raised);
  }
  .thumb {
    position: absolute;
    top: 3px;
    bottom: 3px;
    left: 3px;
    width: calc(50% - 3px);
    border-radius: 7px;
    background: var(--surface);
    border: 1px solid var(--line);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.12);
    transform: translateX(calc(var(--i) * 100%));
    transition: transform 280ms cubic-bezier(0.32, 0.72, 0, 1);
  }
  .tabs button {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 30px;
    border-radius: 7px;
    font-weight: 600;
    color: var(--text-dim);
    transition: color var(--t-fast) var(--ease-out);
  }
  .tabs button.on {
    color: inherit;
  }
  .live {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    animation: blink 1.6s ease-in-out infinite;
  }
  .live.alert {
    background: var(--danger);
    animation-duration: 0.8s;
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    display: grid;
  }
  /* Both panes share one grid cell while they cross. */
  .slot {
    grid-area: 1 / 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
</style>
