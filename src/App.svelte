<script lang="ts">
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { initSettings, store } from "./lib/settings.svelte";
  import Dock from "./views/Dock.svelte";
  import Card from "./views/Card.svelte";
  import Preview from "./views/Preview.svelte";
  import Settings from "./views/Settings.svelte";

  const label = getCurrentWebviewWindow().label;
  const ready = initSettings();

  const media = window.matchMedia("(prefers-color-scheme: dark)");
  let systemDark = $state(media.matches);
  media.addEventListener("change", (e) => (systemDark = e.matches));

  $effect(() => {
    const s = store.s;
    if (!s) return;
    const root = document.documentElement;
    root.dataset.theme = s.theme === "system" ? (systemDark ? "dark" : "light") : s.theme;
    root.dataset.size = s.size;
    root.dataset.edge = s.edge;
  });
</script>

{#await ready then}
  {#if label === "dock"}
    <Dock />
  {:else if label === "card"}
    <Card />
  {:else if label === "preview"}
    <Preview />
  {:else if label === "settings"}
    <Settings />
  {/if}
{/await}
