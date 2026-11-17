<script lang="ts">
  import Icon from "../../lib/Icon.svelte";
  import { ticker, timers, timersAction } from "../../lib/live.svelte";
  import { openCard } from "../../lib/popups";
  import { countdown } from "../../lib/format";
  import { TIMER_CARD, timerFraction, timerLeft, timerPhase } from "./timer";

  const live = timers();
  const t = $derived(live.v.timer);
  const phase = $derived(timerPhase(t));
  const clock = ticker(250, () => phase === "running");
  const left = $derived(timerLeft(t, clock.now));
  const frac = $derived(timerFraction(t, clock.now));

  let el: HTMLButtonElement;
  function click() {
    // While the alarm rings, a click on the bar silences it.
    if (phase === "ringing") timersAction("timerDismiss");
    else openCard("timer", el, TIMER_CARD.width, TIMER_CARD.height);
  }
  const title = $derived(
    phase === "ringing" ? "Time's up — click to dismiss" : phase === "idle" ? "Timer" : `Timer: ${countdown(left)} left`,
  );
</script>

<button
  class="tile glyph timer"
  class:ringing={phase === "ringing"}
  class:paused={phase === "paused"}
  bind:this={el}
  onclick={click}
  data-nodrag
  {title}
>
  <span class="ring" class:active={phase !== "idle"} style:--p={phase === "ringing" ? 1 : frac}>
    <Icon name="hourglass" size="calc(var(--glyph) - 4px)" />
  </span>
  {#if phase !== "idle"}
    <span class="value">{phase === "ringing" ? "0:00" : countdown(left)}</span>
  {/if}
</button>

<style>
  .ring {
    position: relative;
    display: grid;
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
