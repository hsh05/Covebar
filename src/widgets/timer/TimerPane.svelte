<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Icon from "../../lib/Icon.svelte";
  import { ticker, timers, timersAction } from "../../lib/live.svelte";
  import { store } from "../../lib/settings.svelte";
  import { countdown, presetLabel } from "../../lib/format";
  import { timerFraction, timerLeft, timerPhase } from "./timer";

  const live = timers();
  const t = $derived(live.v.timer);
  const phase = $derived(timerPhase(t));
  const clock = ticker(100, () => phase === "running");
  const left = $derived(timerLeft(t, clock.now));
  const frac = $derived(phase === "ringing" ? 0 : timerFraction(t, clock.now));
  const presets = $derived(store.s?.timerPresets ?? [60, 300, 600, 1500, 3600]);

  // The custom duration picker, starting from the last countdown's length.
  const initial = Math.round((live.v.timer.durationMs || 300_000) / 1000);
  let pick = $state({ h: Math.floor(initial / 3600), m: Math.floor((initial % 3600) / 60), s: initial % 60 });
  const pickSeconds = $derived(pick.h * 3600 + pick.m * 60 + pick.s);

  function bump(part: "h" | "m" | "s", d: number) {
    const max = part === "h" ? 23 : 59;
    pick[part] = (pick[part] + d + max + 1) % (max + 1);
  }

  const R = 70;
  const C = 2 * Math.PI * R;
</script>

<div class="pane">

  {#if phase === "idle"}
    <div class="body" in:fade={{ duration: 180 }}>
      <div class="picker">
        {#each [["h", "hr"], ["m", "min"], ["s", "sec"]] as [part, unit] (part)}
          {@const p = part as "h" | "m" | "s"}
          <div class="col" onwheel={(e) => bump(p, e.deltaY < 0 ? 1 : -1)}>
            <button class="icon-btn step" onclick={() => bump(p, 1)} aria-label="More {unit}">
              <svg viewBox="0 0 16 16"><path d="M4 10l4-4 4 4" /></svg>
            </button>
            {#key pick[p]}
              <span class="num" in:fade={{ duration: 120 }}>{String(pick[p]).padStart(2, "0")}</span>
            {/key}
            <span class="unit">{unit}</span>
            <button class="icon-btn step" onclick={() => bump(p, -1)} aria-label="Less {unit}">
              <svg viewBox="0 0 16 16"><path d="M4 6l4 4 4-4" /></svg>
            </button>
          </div>
        {/each}
      </div>
      <div class="presets">
        {#each presets as sec (sec)}
          <button class="chip" onclick={() => timersAction("timerStart", sec)}>{presetLabel(sec)}</button>
        {/each}
      </div>
      <button class="btn primary wide" disabled={pickSeconds === 0} onclick={() => timersAction("timerStart", pickSeconds)}>
        <Icon name="play" size={14} /> Start
      </button>
    </div>
  {:else}
    <div class="body" in:fade={{ duration: 180 }}>
      <div class="dial" class:ringing={phase === "ringing"} in:scale={{ start: 0.9, duration: 240, easing: cubicOut }}>
        <svg viewBox="0 0 160 160">
          <circle class="track" cx="80" cy="80" r={R} />
          <circle
            class="arc"
            cx="80"
            cy="80"
            r={R}
            stroke-dasharray={C}
            stroke-dashoffset={C * (1 - frac)}
            class:smooth={phase === "running"}
          />
        </svg>
        <div class="readout">
          {#if phase === "running" || phase === "paused"}
            <span class="muted-text total" transition:fade={{ duration: 150 }}>of {countdown(t.durationMs)}</span>
          {/if}
          {#if phase === "ringing"}
            <span class="done">Time's up</span>
          {:else}
            <span class="time" class:paused={phase === "paused"}>{countdown(left)}</span>
            {#if phase === "paused"}<span class="state">Paused</span>{/if}
          {/if}
        </div>
      </div>

      <div class="actions">
        {#if phase === "ringing"}
          <button class="btn" onclick={() => timersAction("timerStart", Math.round(t.durationMs / 1000))}>
            <Icon name="reset" size={14} /> Restart
          </button>
          <button class="btn primary" onclick={() => timersAction("timerDismiss")}>
            <Icon name="check" size={14} /> Dismiss
          </button>
        {:else}
          <button class="btn" onclick={() => timersAction("timerReset")} title="Cancel the timer">
            <Icon name="reset" size={14} /> Reset
          </button>
          <button class="btn" onclick={() => timersAction("timerAdd", 60)} title="Add a minute">
            <Icon name="plus" size={14} /> 1 min
          </button>
          {#if phase === "running"}
            <button class="btn primary" onclick={() => timersAction("timerPause")}>
              <Icon name="pause" size={14} /> Pause
            </button>
          {:else}
            <button class="btn primary" onclick={() => timersAction("timerResume")}>
              <Icon name="play" size={14} /> Resume
            </button>
          {/if}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .total {
    margin-bottom: 4px;
    font: 400 12px var(--font);
    font-variant-numeric: tabular-nums;
  }
  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    min-height: 0;
  }
  .picker {
    display: flex;
    gap: 8px;
  }
  .col {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 72px;
    padding: 4px 0;
    border-radius: 10px;
    background: var(--raised);
  }
  .step {
    width: 40px;
    height: 24px;
  }
  .step svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .num {
    font: 600 30px/1.1 var(--font-display);
    font-variant-numeric: tabular-nums;
  }
  .unit {
    font-size: 11px;
    color: var(--text-faint);
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px;
  }
  .wide {
    width: 100%;
    margin-top: auto;
  }
  .dial {
    position: relative;
    width: 168px;
    height: 168px;
  }
  .dial svg {
    width: 100%;
    height: 100%;
    transform: rotate(-90deg);
  }
  .track,
  .arc {
    fill: none;
    stroke-width: 7;
  }
  .track {
    stroke: var(--line);
  }
  .arc {
    stroke: var(--accent);
    stroke-linecap: round;
    transition: stroke-dashoffset var(--t-med) var(--ease-out);
  }
  .arc.smooth {
    transition: stroke-dashoffset 100ms linear;
  }
  .readout {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }
  .time {
    font: 600 34px/1 var(--font-display);
    font-variant-numeric: tabular-nums;
    transition: opacity var(--t-med) var(--ease-out);
  }
  .time.paused {
    opacity: 0.55;
  }
  .state {
    margin-top: 4px;
    font-size: 12px;
    color: var(--text-dim);
  }
  .done {
    font: 600 22px var(--font-display);
    color: var(--danger);
  }
  .ringing {
    animation: shake 0.9s var(--ease-out) infinite;
  }
  @keyframes shake {
    0%,
    60%,
    100% {
      transform: none;
    }
    10%,
    30%,
    50% {
      transform: rotate(-3deg);
    }
    20%,
    40% {
      transform: rotate(3deg);
    }
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: auto;
  }
</style>
