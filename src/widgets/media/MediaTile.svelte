<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { slide, fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Icon from "../../lib/Icon.svelte";
  import { media } from "../../lib/live.svelte";
  import { openCard } from "../../lib/popups";

  const live = media();
  const m = $derived(live.v);
  let el = $state<HTMLElement>();

  function open() {
    if (el) openCard("media", el, 360, 232);
  }
  const tip = $derived(m ? [m.title, m.artist].filter(Boolean).join(" — ") : "");
</script>

{#if m}
  <div class="media" bind:this={el} data-nodrag transition:slide={{ axis: "x", duration: 240, easing: cubicOut }}>
    <button class="tile info" onclick={open} title={tip}>
      <span class="art">
        {#key m.art}
          {#if m.art}
            <img src={m.art} alt="" in:fade={{ duration: 200 }} />
          {:else}
            <span class="noart" in:fade={{ duration: 200 }}><Icon name="music" size={14} /></span>
          {/if}
        {/key}
      </span>
      <span class="text">
        <span class="title">{m.title || m.sourceName}</span>
        <span class="artist">{m.artist || m.sourceName}</span>
      </span>
    </button>
    <button
      class="icon-btn play"
      disabled={!m.canPlayPause}
      onclick={() => invoke("media_command", { action: "playPause" })}
      aria-label={m.playing ? "Pause" : "Play"}
    >
      {#key m.playing}
        <span class="swap" in:fade={{ duration: 160 }}><Icon name={m.playing ? "pause" : "play"} size={15} /></span>
      {/key}
    </button>
  </div>
{/if}

<style>
  .media {
    display: flex;
    align-items: center;
    gap: 2px;
    height: calc(var(--bar) - 8px);
    overflow: hidden;
  }
  .info {
    gap: 9px;
    padding: 0 8px 0 6px;
    max-width: 210px;
    justify-content: flex-start;
  }
  .art {
    position: relative;
    flex: none;
    width: calc(var(--bar) - 20px);
    height: calc(var(--bar) - 20px);
    border-radius: 6px;
    overflow: hidden;
    background: var(--raised);
  }
  .art > * {
    position: absolute;
    inset: 0;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .noart {
    display: grid;
    place-items: center;
    color: var(--text-dim);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    text-align: left;
    line-height: 1.25;
  }
  .title,
  .artist {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title {
    font-weight: 600;
    font-size: 12px;
  }
  .artist {
    font-size: 11px;
    color: var(--text-dim);
  }
  .play {
    width: 30px;
    height: 30px;
  }
  .swap {
    display: grid;
  }
</style>
