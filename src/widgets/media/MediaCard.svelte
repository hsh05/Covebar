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

  // ---- scrubbing: click or drag along the bar to pick a spot in the track
  let scrub = $state<number | null>(null); // seconds under the pointer while dragging
  let barEl = $state<HTMLElement>();
  const shown = $derived(scrub ?? pos);

  function at(e: PointerEvent) {
    const r = barEl!.getBoundingClientRect();
    return Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)) * (m?.duration ?? 0);
  }
  function scrubDown(e: PointerEvent) {
    if (e.button !== 0 || !m?.canSeek) return;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    scrub = at(e);
  }
  function scrubMove(e: PointerEvent) {
    if (scrub != null) scrub = at(e);
  }
  function scrubUp() {
    if (scrub == null) return;
    invoke("media_seek", { seconds: scrub });
    // Keep showing the picked spot until the player confirms it.
    const picked = scrub;
    setTimeout(() => scrub === picked && (scrub = null), 300);
  }
  function scrubKey(e: KeyboardEvent) {
    if (!m?.canSeek) return;
    const step = e.key === "ArrowRight" ? 5 : e.key === "ArrowLeft" ? -5 : 0;
    if (!step) return;
    e.preventDefault();
    invoke("media_seek", { seconds: Math.min(m.duration, Math.max(0, pos + step)) });
  }
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
        <div
          class="scrub"
          class:can={m.canSeek}
          class:active={scrub != null}
          bind:this={barEl}
          role="slider"
          tabindex={m.canSeek ? 0 : -1}
          aria-label="Playback position"
          aria-valuemin={0}
          aria-valuemax={Math.round(m.duration)}
          aria-valuenow={Math.round(shown)}
          aria-valuetext={mmss(shown)}
          title={m.canSeek ? "" : `${m.sourceName} doesn't allow seeking`}
          onpointerdown={scrubDown}
          onpointermove={scrubMove}
          onpointerup={scrubUp}
          onpointercancel={() => (scrub = null)}
          onkeydown={scrubKey}
        >
          <div class="bar">
            <div class="fill" style:width="{(shown / m.duration) * 100}%"></div>
          </div>
          <div class="knob" style:left="{(shown / m.duration) * 100}%"></div>
        </div>
        <div class="times"><span>{mmss(shown)}</span><span>{mmss(m.duration)}</span></div>
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
  /* Tall invisible hit area around the thin bar so it's easy to grab. */
  .scrub {
    position: relative;
    height: 14px;
    display: flex;
    align-items: center;
    outline: none;
  }
  .bar {
    width: 100%;
    height: 4px;
    border-radius: 2px;
    background: var(--line);
    overflow: hidden;
    transition: height var(--t-fast) var(--ease-out);
  }
  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
    transition: width 500ms linear;
  }
  .active .fill,
  .active .knob {
    transition: none;
  }
  .knob {
    position: absolute;
    top: 50%;
    width: 12px;
    height: 12px;
    margin-left: -6px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
    transform: translateY(-50%) scale(0);
    transition:
      transform var(--t-fast) var(--ease-out),
      left 500ms linear;
  }
  .scrub.can:hover .bar,
  .scrub.can:focus-visible .bar,
  .scrub.active .bar {
    height: 6px;
  }
  .scrub.can:hover .knob,
  .scrub.can:focus-visible .knob,
  .scrub.active .knob {
    transform: translateY(-50%) scale(1);
  }
  .scrub.active .knob {
    transform: translateY(-50%) scale(1.2);
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
