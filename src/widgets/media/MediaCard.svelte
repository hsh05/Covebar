<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fade } from "svelte/transition";
  import Icon from "../../lib/Icon.svelte";
  import { media, ticker } from "../../lib/live.svelte";
  import { iconUrl } from "../../lib/icons";
  import { mmss } from "../../lib/format";

  const live = media();
  const m = $derived(live.v);
  const clock = ticker(500, () => !!m?.playing);

  let sourceIcon = $state<string | null>(null);
  $effect(() => {
    const key = m?.sourceIcon;
    sourceIcon = null;
    if (key) iconUrl(key, 14).then((u) => (sourceIcon = u));
  });

  const pos = $derived.by(() => {
    if (!m || m.duration <= 0) return 0;
    const p = m.playing ? m.position + (clock.now - m.updatedAt) / 1000 : m.position;
    return Math.min(m.duration, Math.max(0, p));
  });

  const cmd = (action: string) => invoke("media_command", { action });
</script>

<div class="card">
  {#if m}
    <div class="top">
      <div class="art">
        {#key m.art}
          {#if m.art}
            <img src={m.art} alt="" in:fade={{ duration: 220 }} />
          {:else}
            <span class="noart" in:fade={{ duration: 220 }}><Icon name="music" size={32} /></span>
          {/if}
        {/key}
      </div>
      <div class="meta">
        <div class="source">
          {#if sourceIcon}<img src={sourceIcon} alt="" width="14" height="14" />{/if}
          <span>{m.sourceName}</span>
        </div>
        {#key m.title}
          <div class="title" in:fade={{ duration: 200 }}>{m.title}</div>
        {/key}
        <div class="artist">{m.artist}</div>
        {#if m.album && m.album !== m.title}<div class="album">{m.album}</div>{/if}
      </div>
    </div>

    {#if m.duration > 0}
      <div class="progress">
        <div class="bar"><div class="fill" style:width="{(pos / m.duration) * 100}%"></div></div>
        <div class="times"><span>{mmss(pos)}</span><span>{mmss(m.duration)}</span></div>
      </div>
    {/if}

    <div class="controls">
      <button class="icon-btn" disabled={!m.canPrev} onclick={() => cmd("prev")} aria-label="Previous">
        <Icon name="prev" size={18} />
      </button>
      <button
        class="btn primary round big"
        disabled={!m.canPlayPause}
        onclick={() => cmd("playPause")}
        aria-label={m.playing ? "Pause" : "Play"}
      >
        <Icon name={m.playing ? "pause" : "play"} size={18} />
      </button>
      <button class="icon-btn" disabled={!m.canNext} onclick={() => cmd("next")} aria-label="Next">
        <Icon name="next" size={18} />
      </button>
    </div>
  {:else}
    <div class="empty muted-text">
      <Icon name="music" size={28} />
      <span>Nothing is playing</span>
    </div>
  {/if}
</div>

<style>
  .card {
    padding: 16px;
    gap: 14px;
  }
  .top {
    display: flex;
    gap: 14px;
    min-height: 0;
  }
  .art {
    position: relative;
    flex: none;
    width: 104px;
    height: 104px;
    border-radius: 10px;
    overflow: hidden;
    background: var(--raised);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.25);
  }
  .art > * {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .art img {
    object-fit: cover;
  }
  .noart {
    display: grid;
    place-items: center;
    color: var(--text-faint);
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    padding-top: 2px;
  }
  .source {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-faint);
    margin-bottom: 4px;
  }
  .title {
    font: 600 15px/1.25 var(--font-display);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .artist,
  .album {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-dim);
  }
  .album {
    font-size: 12px;
    color: var(--text-faint);
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--line);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
    transition: width 500ms linear;
  }
  .times {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }
  .controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 18px;
    margin-top: auto;
  }
  .big {
    width: 44px;
    height: 44px;
  }
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }
</style>
