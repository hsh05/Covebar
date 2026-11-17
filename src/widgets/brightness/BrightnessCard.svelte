<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "../../lib/Icon.svelte";
  import { displays } from "../../lib/live.svelte";

  const live = displays();
  const list = $derived(live.v);
</script>

<div class="card">
  <div class="card-title">
    <Icon name="sun" size={16} />
    <span class="grow">Brightness</span>
  </div>

  {#each list as d (d.id)}
    <div class="row">
      <div class="label">
        <span class="name">{d.name}</span>
        <span class="pct">{d.brightness}%</span>
      </div>
      <input
        class="range"
        type="range"
        min="0"
        max="100"
        value={d.brightness}
        style:--val={d.brightness}
        oninput={(e) => invoke("set_brightness", { id: d.id, value: +e.currentTarget.value })}
        aria-label="{d.name} brightness"
      />
    </div>
  {:else}
    <p class="none">
      {live.ready
        ? "No adjustable displays found. External monitors need DDC/CI switched on in their on-screen menu."
        : "Looking for displays…"}
    </p>
  {/each}
</div>

<style>
  .row {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .label {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pct {
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .none {
    margin: 0;
    font-size: 12px;
    color: var(--text-dim);
  }
</style>
