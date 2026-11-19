<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import Icon, { type IconName } from "../lib/Icon.svelte";
  import { updateSettings } from "../lib/settings.svelte";

  const steps: { icon: IconName; title: string; body: string }[] = [
    {
      icon: "star",
      title: "Welcome to Covebar",
      body: "Your apps, media and widgets live on this bar. Click a widget to open its card.",
    },
    {
      icon: "open",
      title: "Right-click for more",
      body: "Every app and widget has its own menu: pin apps, pause the timer, swap currencies and more.",
    },
    {
      icon: "sliders",
      title: "Make it yours",
      body: "Right-click a widget and choose Customize widgets to turn them on or off and drag them into order.",
    },
    {
      icon: "flag",
      title: "Stay out of the way",
      body: "Turn on auto-hide in Settings. The bar slides away and comes back when you touch the edge or open Start.",
    },
  ];

  let open = $state(false);
  let out = $state(false);
  let i = $state(0);
  let dir = $state(1);
  // Bumped on every show so the entrance animation replays.
  let count = $state(0);

  // This window is made on demand: say when it can take "tour-show".
  listen("tour-show", () => {
    i = 0;
    dir = 1;
    out = false;
    open = true;
    count++;
  }).then(() => invoke("popup_ready"));

  function go(to: number) {
    if (to < 0 || to >= steps.length || to === i) return;
    dir = to > i ? 1 : -1;
    i = to;
  }

  function finish() {
    if (out) return;
    out = true;
    updateSettings({ tourDone: true }).catch(() => {});
    setTimeout(() => {
      open = false;
      invoke("hide_tour");
    }, 220);
  }

  function onkey(e: KeyboardEvent) {
    if (!open || out) return;
    if (e.key === "Escape") finish();
    else if (e.key === "ArrowRight") go(i + 1);
    else if (e.key === "ArrowLeft") go(i - 1);
    else if (e.key === "Enter") i === steps.length - 1 ? finish() : go(i + 1);
    else return;
    e.preventDefault();
  }

  const last = $derived(i === steps.length - 1);
</script>

<svelte:window onkeydown={onkey} />

{#key count}
  {#if open}
    <div class="surface tour" class:out role="dialog" aria-label="Covebar tour">
      <div class="head">
        <span class="count">{i + 1} of {steps.length}</span>
        <button class="icon-btn x" onclick={finish} aria-label="Skip the tour">
          <Icon name="close" size={14} />
        </button>
      </div>
      <div class="stage">
        {#key i}
          <div class="step" style:--dir={dir}>
            <span class="badge"><Icon name={steps[i].icon} size={22} /></span>
            <div class="text">
              <span class="title">{steps[i].title}</span>
              <span class="body">{steps[i].body}</span>
            </div>
          </div>
        {/key}
      </div>
      <div class="foot">
        <div class="dots" role="tablist">
          {#each steps as s, n (s.title)}
            <button
              class="dot"
              class:on={n === i}
              role="tab"
              aria-selected={n === i}
              aria-label={s.title}
              onclick={() => go(n)}
            ></button>
          {/each}
        </div>
        <div class="nav">
          {#if i > 0}
            <button class="btn" onclick={() => go(i - 1)}>Back</button>
          {:else}
            <button class="btn" onclick={finish}>Skip</button>
          {/if}
          <button class="btn primary" onclick={() => (last ? finish() : go(i + 1))}>
            {#if last}<Icon name="check" size={14} /> Done{:else}Next{/if}
          </button>
        </div>
      </div>
    </div>
  {/if}
{/key}

<style>
  .tour {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px 14px 16px;
    animation: tour-in 460ms cubic-bezier(0.34, 1.4, 0.64, 1) both;
  }
  .tour.out {
    animation: pop-out 200ms var(--ease-in) both;
    pointer-events: none;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .count {
    font-size: 12px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .x {
    width: 26px;
    height: 26px;
  }
  .stage {
    display: grid;
    flex: 1;
    overflow: hidden;
  }
  .step {
    grid-area: 1 / 1;
    display: flex;
    gap: 14px;
    align-items: flex-start;
    animation: step-in 380ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  .badge {
    display: grid;
    place-items: center;
    flex: none;
    width: 46px;
    height: 46px;
    border-radius: 14px;
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent);
    animation: badge-pop 520ms 80ms cubic-bezier(0.34, 1.6, 0.64, 1) both;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .title {
    font: 600 18px/1.2 var(--font-display);
  }
  .body {
    font-size: 13px;
    line-height: 1.45;
    color: var(--text-dim);
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .dots {
    display: flex;
    gap: 6px;
  }
  .dot {
    width: 7px;
    height: 7px;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: color-mix(in srgb, var(--text) 25%, transparent);
    cursor: pointer;
    transition:
      width 260ms cubic-bezier(0.22, 1, 0.36, 1),
      background 200ms;
  }
  .dot.on {
    width: 20px;
    background: var(--accent);
  }
  .nav {
    display: flex;
    gap: 8px;
  }
  .nav .btn {
    min-width: 72px;
  }
  @keyframes tour-in {
    from {
      opacity: 0;
      transform: translateY(calc(var(--rise) * 1.6)) scale(0.92);
    }
  }
  @keyframes step-in {
    from {
      opacity: 0;
      transform: translateX(calc(var(--dir) * 28px));
    }
  }
  @keyframes badge-pop {
    from {
      transform: scale(0.6) rotate(-12deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .tour,
    .step,
    .badge {
      animation-duration: 1ms;
    }
  }
</style>
