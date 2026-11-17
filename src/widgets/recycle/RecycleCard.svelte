<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onDestroy } from "svelte";
  import { fade } from "svelte/transition";
  import Icon from "../../lib/Icon.svelte";
  import { recycle } from "../../lib/live.svelte";
  import { bytes } from "../../lib/format";

  const live = recycle();
  const bin = $derived(live.v);
  let confirming = $state(false);
  let busy = $state(false);
  let error = $state("");
  let progress = $state({ fraction: 0, deleted: 0, total: 0 });
  // Snapshot of what's being deleted, since the live numbers shrink as it goes.
  let job = $state({ items: 0, bytes: 0 });

  const un = listen<typeof progress>("recycle-progress", (e) => {
    if (busy) progress = e.payload;
  });
  onDestroy(() => un.then((f) => f()));

  async function empty() {
    job = { ...bin };
    progress = { fraction: 0, deleted: 0, total: bin.items };
    busy = true;
    error = "";
    try {
      await invoke("empty_recycle");
      progress = { ...progress, fraction: 1, deleted: progress.total };
      // Let the bar visibly reach the end before switching back.
      await new Promise((r) => setTimeout(r, 450));
      confirming = false;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
  const plural = $derived(`${bin.items} item${bin.items === 1 ? "" : "s"}`);
</script>

<div class="card">
  <div class="card-title">
    <Icon name={bin.items ? "binFull" : "bin"} size={16} />
    <span class="grow">Recycle Bin</span>
  </div>

  {#if busy}
    <div class="body" in:fade={{ duration: 160 }}>
      <p class="ask">Emptying…</p>
      <p class="muted-text small">
        {progress.deleted} of {job.items} item{job.items === 1 ? "" : "s"} · {bytes(job.bytes)}
      </p>
      <div
        class="bar"
        class:waiting={progress.fraction === 0}
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(progress.fraction * 100)}
      >
        <div class="fill" style:width="{progress.fraction * 100}%"></div>
      </div>
      <p class="pct">{Math.round(progress.fraction * 100)}%</p>
    </div>
  {:else if confirming && bin.items}
    <div class="body" in:fade={{ duration: 160 }}>
      <p class="ask">Permanently delete {plural}?</p>
      <p class="muted-text small">This frees {bytes(bin.bytes)} and can't be undone.</p>
      {#if error}<p class="error small">{error}</p>{/if}
      <div class="actions">
        <button class="btn" disabled={busy} onclick={() => (confirming = false)}>Cancel</button>
        <button class="btn primary danger-fill" disabled={busy} onclick={empty}>
          {busy ? "Emptying…" : "Empty"}
        </button>
      </div>
    </div>
  {:else}
    <div class="body" in:fade={{ duration: 160 }}>
      {#if bin.items}
        <p class="big">{plural}</p>
        <p class="muted-text small">{bytes(bin.bytes)} on disk</p>
      {:else}
        <p class="big">Empty</p>
        <p class="muted-text small">Nothing to clean up</p>
      {/if}
      <div class="actions">
        <button class="btn" onclick={() => invoke("open_recycle")}><Icon name="open" size={14} /> Open</button>
        <button class="btn danger" disabled={!bin.items} onclick={() => (confirming = true)}>
          <Icon name="bin" size={14} /> Empty…
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  p {
    margin: 0;
  }
  .big {
    font: 600 22px var(--font-display);
  }
  .ask {
    font: 600 15px var(--font-display);
  }
  .small {
    font-size: 12px;
  }
  .error {
    color: var(--danger);
  }
  .actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-top: auto;
  }
  .bar {
    position: relative;
    height: 8px;
    margin-top: 14px;
    border-radius: 4px;
    background: var(--pressed);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: inherit;
    background: var(--danger);
    transition: width 220ms var(--ease-out);
  }
  /* Before the first files are gone, sweep so it never looks stuck. */
  .waiting::after {
    content: "";
    position: absolute;
    inset: 0;
    width: 35%;
    border-radius: inherit;
    background: var(--danger);
    opacity: 0.6;
    animation: sweep 1.1s var(--ease-out) infinite;
  }
  @keyframes sweep {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }
  .pct {
    margin-top: 6px;
    font-size: 12px;
    color: var(--text-dim);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .danger-fill {
    background: var(--danger);
  }
  .danger-fill:hover:not(:disabled) {
    background: var(--danger);
  }
</style>
