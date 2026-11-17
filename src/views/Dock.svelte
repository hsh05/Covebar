<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getName } from "@tauri-apps/api/app";
  import { showMenu } from "../lib/menu";
  import { store, updateSettings } from "../lib/settings.svelte";
  import { WIDGETS } from "../lib/widgets";
  import type { Edge } from "../lib/types";

  let appName = $state("Covebar");
  getName().then((n) => (appName = n));

  const widgets = $derived(
    (store.s?.widgets ?? []).filter((w) => w.enabled && WIDGETS[w.id]).map((w) => WIDGETS[w.id]),
  );
  const edge = $derived<Edge>(store.s?.edge ?? "bottom");

  // Drag the bar's empty space up or down to snap it to the other edge.
  let drag = $state<{ startY: number; target: Edge; active: boolean } | null>(null);

  function down(e: PointerEvent) {
    if (e.button !== 0 || (e.target as Element).closest("[data-nodrag]")) return;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    drag = { startY: e.screenY, target: edge, active: false };
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    if (Math.abs(e.screenY - drag.startY) > 16) drag.active = true;
    if (drag.active) drag.target = e.screenY < window.screen.height / 2 ? "top" : "bottom";
  }
  function up() {
    const d = drag;
    drag = null;
    if (d?.active && d.target !== edge) updateSettings({ edge: d.target });
  }

  async function menu(e: MouseEvent) {
    e.preventDefault();
    const other: Edge = edge === "bottom" ? "top" : "bottom";
    await showMenu([
      { label: "Task Manager", icon: "activity", action: () => invoke("launch_app", { target: "taskmgr.exe" }) },
      { label: `${appName} settings`, icon: "settings", action: () => invoke("open_settings") },
      { label: `Move bar to the ${other}`, icon: "swap", action: () => updateSettings({ edge: other }) },
      { separator: true },
      { label: `Quit ${appName}`, icon: "power", danger: true, action: () => invoke("quit") },
    ]);
  }
</script>

<main
  class="bar"
  class:top={edge === "top"}
  class:dragging={drag?.active}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  oncontextmenu={menu}
>
  {#each widgets as w (w.id)}
    <div class="slot" class:grow={w.grow}>
      <w.tile />
    </div>
  {/each}

  {#if drag?.active}
    <div class="hint">
      {drag.target === edge ? "Drag towards the other edge" : `Release to move the bar to the ${drag.target}`}
    </div>
  {/if}
</main>

<style>
  .bar {
    position: relative;
    height: 100vh;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 6px;
    background: var(--tint);
    border-top: 1px solid var(--line);
  }
  .bar.top {
    border-top: 0;
    border-bottom: 1px solid var(--line);
  }
  .slot {
    display: flex;
    align-items: center;
    height: 100%;
    flex: none;
  }
  /* A widget with nothing to show (e.g. no media playing) takes no space. */
  .slot:empty {
    display: none;
  }
  .slot.grow {
    flex: 1;
    min-width: 0;
  }
  .dragging {
    cursor: grabbing;
  }
  .hint {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--card-tint);
    color: var(--accent);
    font-weight: 600;
    pointer-events: none;
  }
</style>
