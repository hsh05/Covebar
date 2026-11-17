<script lang="ts">
  import { scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Icon from "../../lib/Icon.svelte";
  import { ticker, timers, timersAction } from "../../lib/live.svelte";
  import { openCard } from "../../lib/popups";
  import { countdown, duration } from "../../lib/format";
  import { TIMER_CARD, timerFraction, timerLeft, timerPhase } from "./timer";

  const live = timers();
  const t = $derived(live.v.timer);
  const phase = $derived(timerPhase(t));
  const clock = ticker(250, () => phase === "running");
  const left = $derived(timerLeft(t, clock.now));
  const frac = $derived(timerFraction(t, clock.now));

  // With no countdown going, the tile shows the stopwatch if that is in use.
  const sw = $derived(live.v.stopwatch);
  const swRunning = $derived(sw.startedAt != null);
  const showSw = $derived(phase === "idle" && (swRunning || sw.elapsedMs > 0));
  const swClock = ticker(200, () => swRunning && phase === "idle");
  const swElapsed = $derived(sw.elapsedMs + (sw.startedAt != null ? swClock.now - sw.startedAt : 0));

  let el: HTMLButtonElement;
  function click() {
    // While the alarm rings, a click on the bar silences it.
    if (phase === "ringing") timersAction("timerDismiss");
    else openCard("timer", el, TIMER_CARD.width, TIMER_CARD.height);
  }
  const title = $derived(
    phase === "ringing"
      ? "Time's up — click to dismiss"
      : phase !== "idle"
        ? `Timer: ${countdown(left)} left`
        : showSw
          ? `Stopwatch: ${duration(swElapsed)}`
          : "Timer & stopwatch",
  );
</script>

<button
  class="tile glyph timer"
  class:ringing={phase === "ringing"}
  class:paused={phase === "paused" || (showSw && !swRunning)}
  class:sw-running={showSw && swRunning}
  bind:this={el}
  onclick={click}
  data-nodrag
  {title}
>
  <span class="ring" class:active={phase !== "idle"} style:--p={phase === "ringing" ? 1 : frac}>
    {#key showSw}
      <span class="glyph-swap" in:scale={{ start: 0.5, duration: 220, easing: cubicOut }}>
        <Icon name={showSw ? "stopwatch" : "hourglass"} size="calc(var(--glyph) - 4px)" />
      </span>
    {/key}
  </span>
  {#if phase !== "idle"}
    <span class="value">{phase === "ringing" ? "0:00" : countdown(left)}</span>
  {:else if showSw}
    <span class="value">{duration(swElapsed)}</span>
  {/if}
</button>

<style>
  .ring {
    position: relative;
    display: grid;
    grid-template-areas: "a";
    place-items: center;
    width: calc(var(--glyph) + 6px);
    height: calc(var(--glyph) + 6px);
  }
  .ring::before {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: conic-gradient(var(--accent) calc(var(--p) * 1turn), var(--line) 0);
    mask: radial-gradient(farthest-side, transparent calc(100% - 2.2px), #000 calc(100% - 2px));
    opacity: 0;
    transform: scale(0.7);
    transition:
      opacity var(--t-med) var(--ease-out),
      transform var(--t-med) var(--ease-out);
  }
  .ring.active::before {
    opacity: 1;
    transform: none;
  }
  .glyph-swap {
    grid-area: 1 / 1;
    display: grid;
  }
  .sw-running .glyph-swap {
    color: var(--accent);
  }
  .paused .value {
    opacity: 0.55;
  }
  .ringing {
    color: var(--danger);
    animation: ring-pulse 1s var(--ease-out) infinite;
  }
  .ringing .ring::before {
    background: var(--danger);
  }
  .ringing .value {
    color: var(--danger);
  }
  @keyframes ring-pulse {
    50% {
      background: color-mix(in srgb, var(--danger) 18%, transparent);
    }
  }
</style>
