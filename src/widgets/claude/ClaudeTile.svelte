<script lang="ts">
  import { claude } from "../../lib/live.svelte";
  import { openCard } from "../../lib/popups";
  import { span } from "../../lib/format";

  const live = claude();
  const u = $derived(live.v);
  const session = $derived(u.windows.find((w) => w.key === "session") ?? u.windows[0] ?? null);
  const pct = $derived(session ? Math.round(session.percent) : null);
  const stale = $derived(u.status !== "ok");
  const level = $derived(pct == null ? "" : pct >= 90 ? "hot" : pct >= 70 ? "warm" : "");

  const R = 7;
  const C = 2 * Math.PI * R;

  const title = $derived.by(() => {
    if (!session) return u.message ?? "Claude usage";
    const reset = session.resetsAt ? `, resets in ${span(Date.parse(session.resetsAt) - Date.now())}` : "";
    return `Claude ${session.label.toLowerCase()}: ${pct}% used${reset}${stale && u.message ? `\n${u.message}` : ""}`;
  });

  let el: HTMLButtonElement;
  function open() {
    openCard("claude", el, 320, 104 + Math.max(2, u.windows.length) * 62 + (stale ? 44 : 0));
  }
</script>

<button class="tile glyph {level}" class:stale bind:this={el} onclick={open} data-nodrag {title}>
  <svg class="ring" viewBox="0 0 20 20" aria-hidden="true">
    <circle class="track" cx="10" cy="10" r={R} />
    <circle
      class="arc"
      cx="10"
      cy="10"
      r={R}
      stroke-dasharray={C}
      stroke-dashoffset={C * (1 - (pct ?? 0) / 100)}
    />
  </svg>
  <span class="value">{pct == null ? "–" : `${pct}%`}</span>
</button>

<style>
  .ring {
    width: var(--glyph);
    height: var(--glyph);
    transform: rotate(-90deg);
    fill: none;
    stroke-width: 2.6;
  }
  .track {
    stroke: var(--pressed);
  }
  .arc {
    stroke: var(--accent);
    stroke-linecap: round;
    transition:
      stroke-dashoffset 700ms var(--ease-out),
      stroke 300ms var(--ease-out);
  }
  .warm .arc {
    stroke: #e8a33d;
  }
  .hot .arc {
    stroke: var(--danger);
  }
  .stale .arc {
    opacity: 0.55;
  }
  .value {
    min-width: 3ch;
  }
</style>
