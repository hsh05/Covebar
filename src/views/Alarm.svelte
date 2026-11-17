<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import Icon from "../lib/Icon.svelte";
  import { ticker, timersAction } from "../lib/live.svelte";
  import { duration, presetLabel } from "../lib/format";

  interface Show {
    session: number;
    durationMs: number;
  }

  let cur = $state<Show | null>(null);
  let since = $state(0);
  let out = $state(false);
  // Bumped on every show so the entrance animation replays.
  let count = $state(0);
  const clock = ticker(1000, () => cur != null && !out);

  listen<Show>("alarm-show", (e) => {
    cur = e.payload;
    since = Date.now();
    out = false;
    count++;
  });
  listen("alarm-out", () => (out = true));

  const label = $derived(cur && cur.durationMs > 0 ? `${presetLabel(Math.round(cur.durationMs / 1000))} timer` : "Timer");
  const ago = $derived(Math.max(0, clock.now - since));

  function act(action: string, value?: number) {
    out = true;
    timersAction(action, value);
  }
</script>

{#key count}
  {#if cur}
    <div class="surface alarm" class:out role="alertdialog" aria-label="Time's up">
      <div class="glow"></div>
      <div class="top">
        <span class="bell">
          <span class="pulse"></span>
          <span class="pulse late"></span>
          <span class="shake"><Icon name="bell" size={22} /></span>
        </span>
        <div class="text">
          <span class="title">Time's up</span>
          <span class="sub">{label} · ended {ago < 1000 ? "just now" : `${duration(ago)} ago`}</span>
        </div>
        <button class="icon-btn x" onclick={() => act("timerDismiss")} aria-label="Dismiss">
          <Icon name="close" size={14} />
        </button>
      </div>
      <div class="actions">
        <button class="btn" onclick={() => act("timerStart", 60)} title="Snooze for a minute">
          <Icon name="plus" size={14} /> 1 min
        </button>
        <button class="btn" onclick={() => act("timerStart", Math.round((cur?.durationMs ?? 60_000) / 1000))}>
          <Icon name="reset" size={14} /> Restart
        </button>
        <button class="btn primary" onclick={() => act("timerDismiss")}>
          <Icon name="check" size={14} /> Dismiss
        </button>
      </div>
    </div>
  {/if}
{/key}

<style>
  .alarm {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 14px 14px 16px;
    animation: alarm-in 420ms cubic-bezier(0.34, 1.4, 0.64, 1) both;
  }
  .alarm.out {
    animation: pop-out 200ms var(--ease-in) both;
    pointer-events: none;
  }
  /* A soft wash of the alert colour that breathes while the alarm rings. */
  .glow {
    position: absolute;
    inset: 0;
    background: radial-gradient(120% 90% at 0% 0%, color-mix(in srgb, var(--danger) 22%, transparent), transparent 60%);
    animation: breathe 2s ease-in-out infinite;
    pointer-events: none;
  }
  .top {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .bell {
    position: relative;
    display: grid;
    place-items: center;
    flex: none;
    width: 42px;
    height: 42px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    color: var(--danger);
  }
  .pulse {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 2px solid var(--danger);
    animation: ripple 1.6s cubic-bezier(0.22, 1, 0.36, 1) infinite;
  }
  .pulse.late {
    animation-delay: 0.8s;
  }
  .shake {
    display: grid;
    transform-origin: 50% 15%;
    animation: ring 1.2s ease-in-out infinite;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title {
    font: 600 18px/1.2 var(--font-display);
  }
  .sub {
    font-size: 12px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .x {
    align-self: flex-start;
    width: 26px;
    height: 26px;
  }
  .actions {
    position: relative;
    display: grid;
    grid-template-columns: 1fr 1fr 1.2fr;
    gap: 8px;
  }
  @keyframes alarm-in {
    from {
      opacity: 0;
      transform: translateY(calc(var(--rise) * 1.6)) scale(0.92);
    }
  }
  @keyframes ripple {
    from {
      opacity: 0.6;
      transform: scale(1);
    }
    to {
      opacity: 0;
      transform: scale(1.7);
    }
  }
  @keyframes ring {
    0%,
    50%,
    100% {
      transform: none;
    }
    8%,
    24%,
    40% {
      transform: rotate(-14deg);
    }
    16%,
    32% {
      transform: rotate(14deg);
    }
  }
  @keyframes breathe {
    50% {
      opacity: 0.45;
    }
  }
</style>
