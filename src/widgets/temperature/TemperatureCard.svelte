<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fly, fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Icon from "../../lib/Icon.svelte";
  import { temps } from "../../lib/live.svelte";

  const live = temps();
  const t = $derived(live.v);

  // Temperatures are drawn on a 30-100 °C scale.
  const tempFrac = (c: number) => Math.max(0, Math.min(1, (c - 30) / 70));
  const level = (c: number | null) => (c == null ? "" : c >= 85 ? "hot" : c >= 70 ? "warm" : "");

  const rows = $derived([
    { key: "cpu", label: "CPU", name: t.cpuName, temp: t.cpuTemp, load: t.cpuLoad },
    { key: "gpu", label: "GPU", name: t.gpuName, temp: t.gpuTemp, load: t.gpuLoad },
    { key: "mem", label: "Memory", name: null, temp: null, load: t.memLoad },
  ].filter((r) => r.temp != null || r.load != null));
</script>

<div class="card">
  <div class="card-title">
    <Icon name="thermo" size={16} />
    <span class="grow">Temperature</span>
    {#if t.source}
      <span class="src muted-text" in:fade>LibreHardwareMonitor</span>
    {/if}
  </div>

  <div class="rows">
    {#each rows as r, i (r.key)}
      <div class="row" in:fly={{ y: 8, duration: 260, delay: i * 50, easing: cubicOut }}>
        <div class="head">
          <span class="label">{r.label}</span>
          {#if r.temp != null}
            <span class="temp {level(r.temp)}">{Math.round(r.temp)}<small>°C</small></span>
          {/if}
        </div>
        {#if r.name}<div class="name muted-text">{r.name}</div>{/if}
        {#if r.temp != null}
          <div class="bar temp-bar"><div class="fill {level(r.temp)}" style:width="{tempFrac(r.temp) * 100}%"></div></div>
        {/if}
        {#if r.load != null}
          <div class="load">
            <div class="bar"><div class="fill load-fill" style:width="{r.load}%"></div></div>
            <span class="pct">{Math.round(r.load)}%</span>
          </div>
        {/if}
      </div>
    {/each}
  </div>

  {#if !t.source}
    <div class="hint" in:fly={{ y: 8, duration: 260, delay: 120, easing: cubicOut }}>
      <p>
        For CPU and GPU temperatures, run <b>LibreHardwareMonitor</b> with
        <i>Options → Remote Web Server</i> on, or run it as administrator.
      </p>
      <button
        class="btn"
        onclick={() =>
          invoke("launch_app", { target: "https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/releases" })}
      >
        <Icon name="open" size={14} /> Get LibreHardwareMonitor
      </button>
    </div>
  {/if}
</div>

<style>
  .src {
    font: 500 11px var(--font);
  }
  .rows {
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
  .name {
    margin-top: -1px;
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .temp {
    font: 600 20px var(--font-display);
    font-variant-numeric: tabular-nums;
    transition: color 300ms var(--ease-out);
  }
  .temp small {
    font-size: 12px;
    color: var(--text-dim);
    margin-left: 1px;
  }
  .warm {
    color: #e8a33d;
  }
  .hot {
    color: var(--danger);
  }
  .bar {
    flex: 1;
    height: 6px;
    margin-top: 6px;
    border-radius: 3px;
    background: var(--pressed);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    transition:
      width 600ms var(--ease-out),
      background-color 300ms var(--ease-out);
  }
  .fill.warm {
    background: #e8a33d;
  }
  .fill.hot {
    background: var(--danger);
  }
  .load-fill {
    background: var(--text-dim);
  }
  .load {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .load .bar {
    margin-top: 0;
    height: 4px;
  }
  .temp-bar + .load {
    margin-top: 6px;
  }
  .pct {
    width: 4ch;
    text-align: right;
    font-size: 12px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .hint {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border-radius: 10px;
    background: var(--accent-soft);
    font-size: 12px;
    line-height: 1.45;
  }
  .hint p {
    margin: 0;
  }
</style>
