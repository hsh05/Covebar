<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fly, fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Icon from "../../lib/Icon.svelte";
  import { claude, ticker } from "../../lib/live.svelte";
  import { ago, span } from "../../lib/format";

  const live = claude();
  const u = $derived(live.v);
  const clock = ticker(30000, () => true);

  let spinning = $state(false);
  async function refresh() {
    spinning = true;
    await invoke("refresh_claude");
    setTimeout(() => (spinning = false), 700);
  }

  const level = (p: number) => (p >= 90 ? "hot" : p >= 70 ? "warm" : "");
  const resets = (iso: string | null) => {
    if (!iso) return "Starts with your next message";
    const ms = Date.parse(iso) - clock.now;
    if (ms <= 0) return "Resetting now";
    const at = new Date(iso);
    const when =
      ms > 20 * 3600e3
        ? at.toLocaleDateString(undefined, { weekday: "short" }) +
          " " +
          at.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })
        : at.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
    return `Resets in ${span(ms)} · ${when}`;
  };
</script>

<div class="card">
  <div class="card-title">
    <Icon name="gauge" size={16} />
    <span class="grow">Claude usage</span>
    {#if u.plan}<span class="plan" in:fade>{u.plan}</span>{/if}
    <button class="icon-btn small" class:spinning onclick={refresh} title="Refresh" aria-label="Refresh">
      <Icon name="reset" size={14} />
    </button>
  </div>

  {#if u.windows.length}
    <div class="list">
      {#each u.windows as w, i (w.key)}
        <div class="win" in:fly={{ y: 8, duration: 260, delay: i * 50, easing: cubicOut }}>
          <div class="head">
            <span class="label">{w.label}</span>
            <span class="pct {level(w.percent)}">{Math.round(w.percent)}%</span>
          </div>
          <div class="bar"><div class="fill {level(w.percent)}" style:width="{w.percent}%"></div></div>
          <div class="reset muted-text">{resets(w.resetsAt)}</div>
        </div>
      {/each}
    </div>
  {:else if u.status === "loading"}
    <div class="empty muted-text" in:fade>Loading your usage…</div>
  {/if}

  {#if u.status !== "ok" && u.status !== "loading" && u.message}
    <div class="note {u.status}" in:fly={{ y: 6, duration: 240, easing: cubicOut }}>{u.message}</div>
  {/if}

  {#if u.fetchedAt}
    <div class="foot muted-text">Updated {ago(u.fetchedAt, clock.now)}</div>
  {/if}
</div>

<style>
  .plan {
    padding: 2px 8px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 11px;
    font-weight: 700;
  }
  .small {
    width: 26px;
    height: 26px;
  }
  .spinning :global(.icon) {
    animation: spin 700ms var(--ease-out);
  }
  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(-360deg);
    }
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .label {
    font-weight: 600;
  }
  .pct {
    font: 600 16px var(--font-display);
    font-variant-numeric: tabular-nums;
    transition: color 300ms var(--ease-out);
  }
  .bar {
    height: 6px;
    margin: 6px 0 4px;
    border-radius: 3px;
    background: var(--pressed);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    transition:
      width 700ms var(--ease-out),
      background-color 300ms var(--ease-out);
  }
  .warm {
    color: #e8a33d;
  }
  .hot {
    color: var(--danger);
  }
  .fill.warm {
    background: #e8a33d;
  }
  .fill.hot {
    background: var(--danger);
  }
  .reset {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  .empty {
    flex: 1;
    display: grid;
    place-items: center;
  }
  .note {
    padding: 9px 12px;
    border-radius: 10px;
    background: var(--raised);
    font-size: 12px;
    line-height: 1.4;
  }
  .note.expired,
  .note.signedOut {
    background: var(--accent-soft);
  }
  .foot {
    margin-top: auto;
    font-size: 11px;
  }
</style>
