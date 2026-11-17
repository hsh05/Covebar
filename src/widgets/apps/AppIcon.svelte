<script lang="ts">
  import { iconUrl } from "../../lib/icons";
  import type { DockApp } from "../../lib/types";

  let { app, size }: { app: DockApp; size: number } = $props();

  let src = $state<string | null>(null);
  $effect(() => {
    let live = true;
    iconUrl(app.icon, size).then((u) => live && (src = u));
    return () => (live = false);
  });
</script>

{#if src}
  <img {src} alt="" width={size} height={size} draggable="false" />
{:else}
  <span class="fallback" style:width="{size}px" style:height="{size}px">{app.name.slice(0, 1)}</span>
{/if}

<style>
  img {
    display: block;
    pointer-events: none;
  }
  .fallback {
    display: grid;
    place-items: center;
    border-radius: 25%;
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 700;
  }
</style>
