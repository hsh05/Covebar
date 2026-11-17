<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fade } from "svelte/transition";
  import Icon from "../../lib/Icon.svelte";
  import { recycle } from "../../lib/live.svelte";
  import { bytes } from "../../lib/format";

  const live = recycle();
  const bin = $derived(live.v);
  let confirming = $state(false);
  let busy = $state(false);
  let error = $state("");

  async function empty() {
    busy = true;
    error = "";
    try {
      await invoke("empty_recycle");
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

  {#if confirming && bin.items}
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
  .danger-fill {
    background: var(--danger);
  }
  .danger-fill:hover:not(:disabled) {
    background: var(--danger);
  }
</style>
