<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "../../lib/Icon.svelte";
  import { audio } from "../../lib/live.svelte";
  import { speakerIcon } from "./VolumeTile.svelte";
  import type { Endpoint } from "../../lib/types";

  const live = audio();
  const st = $derived(live.v);

  const setVol = (which: string, v: number) => invoke("set_volume", { which, volume: v / 100 });
  const toggle = (which: string, ep: Endpoint) => invoke("set_mute", { which, muted: !ep.muted });
</script>

{#snippet level(which: "output" | "input", label: string, ep: Endpoint | null)}
  <div class="level">
    <div class="label">
      <span>{label}</span>
      {#if ep}<span class="dev">{ep.name}</span>{/if}
    </div>
    {#if ep}
      {@const pct = Math.round(ep.volume * 100)}
      <div class="ctl">
        <button
          class="icon-btn"
          class:muted={ep.muted}
          onclick={() => toggle(which, ep)}
          aria-label={ep.muted ? "Unmute" : "Mute"}
          title={ep.muted ? "Unmute" : "Mute"}
        >
          {#if which === "output"}
            <Icon name={speakerIcon(pct, ep.muted)} size={18} />
          {:else}
            <Icon name={ep.muted ? "micOff" : "mic"} size={18} />
          {/if}
        </button>
        <input
          class="range"
          type="range"
          min="0"
          max="100"
          value={pct}
          style:--val={ep.muted ? 0 : pct}
          oninput={(e) => setVol(which, +e.currentTarget.value)}
          aria-label="{label} volume"
        />
        <span class="pct">{pct}</span>
      </div>
    {:else}
      <p class="none">{which === "output" ? "No output device" : "No microphone"}</p>
    {/if}
  </div>
{/snippet}

<div class="card">
  <div class="card-title">
    <Icon name="volume2" size={16} />
    <span class="grow">Sound</span>
    <button class="link" onclick={() => invoke("launch_app", { target: "ms-settings:sound" })}>Sound settings</button>
  </div>

  {@render level("output", "Speakers", st.output)}
  {@render level("input", "Microphone", st.input)}

  {#if st.outputs.length > 1}
    <div class="devices">
      <div class="label"><span>Output device</span></div>
      {#each st.outputs as d (d.id)}
        {@const on = d.id === st.output?.id}
        <button class="device" class:on onclick={() => !on && invoke("set_default_output", { id: d.id })}>
          <span class="dot"></span>
          <span class="dname">{d.name}</span>
          {#if on}<Icon name="check" size={14} />{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .level {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .label {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: 12px;
    font-weight: 600;
  }
  .dev {
    flex: 1;
    min-width: 0;
    font-weight: 400;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: right;
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .icon-btn.muted {
    color: var(--danger);
  }
  .pct {
    width: 26px;
    text-align: right;
    font-size: 12px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .none {
    margin: 4px 0;
    font-size: 12px;
    color: var(--text-faint);
  }
  .devices {
    display: flex;
    flex-direction: column;
    gap: 2px;
    border-top: 1px solid var(--line);
    padding-top: 10px;
  }
  .devices .label {
    margin-bottom: 4px;
  }
  .device {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 34px;
    padding: 0 10px;
    border-radius: 8px;
    text-align: left;
    transition: background-color var(--t-fast) var(--ease-out);
  }
  .device:hover {
    background: var(--hover);
  }
  .device.on {
    background: var(--accent-soft);
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--text-faint);
    transition:
      background-color var(--t-fast) var(--ease-out),
      border-color var(--t-fast) var(--ease-out);
  }
  .on .dot {
    background: var(--accent);
    border-color: var(--accent);
  }
  .dname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .link {
    font: 400 12px var(--font);
    color: var(--accent);
    padding: 2px 4px;
    border-radius: 4px;
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
