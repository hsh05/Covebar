<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { tick } from "svelte";
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import { getName } from "@tauri-apps/api/app";
  import { showMenu } from "../lib/menu";
  import { store, updateSettings } from "../lib/settings.svelte";
  import { WIDGETS } from "../lib/widgets";
  import type { Edge } from "../lib/types";

  let appName = $state("Covebar");
  getName().then((n) => (appName = n));

  const shownIds = $derived((store.s?.widgets ?? []).filter((w) => w.enabled && WIDGETS[w.id]).map((w) => w.id));
  const edge = $derived<Edge>(store.s?.edge ?? "bottom");
  const centered = $derived((store.s?.align ?? "center") === "center");

  // ---- drag a widget along the bar to reorder it; the others slide out of the way
  let order = $state<string[] | null>(null); // live order while dragging
  const ids = $derived(order ?? shownIds);
  const slots: Record<string, HTMLElement> = {};
  let wdrag = $state<{ id: string; pid: number; startX: number; grab: number; active: boolean; dx: number } | null>(
    null,
  );
  let settling = $state<string | null>(null);

  function slotDown(e: PointerEvent, id: string) {
    if (e.button !== 0) return;
    // Keep the bar's own edge drag out of it.
    e.stopPropagation();
    const el = slots[id];
    wdrag = { id, pid: e.pointerId, startX: e.clientX, grab: e.clientX - el.offsetLeft, active: false, dx: 0 };
    window.addEventListener("pointermove", slotMove);
    window.addEventListener("pointerup", slotUp);
    window.addEventListener("pointercancel", slotUp);
  }

  async function slotMove(e: PointerEvent) {
    const d = wdrag;
    if (!d) return;
    const el = slots[d.id];
    if (!d.active) {
      if (Math.abs(e.clientX - d.startX) < 6) return;
      d.active = true;
      order = [...shownIds];
      el.setPointerCapture(d.pid);
      invoke("close_card", { widget: null });
    }
    const bar = el.parentElement!;
    const left = Math.max(0, Math.min(e.clientX - d.grab, bar.clientWidth - el.offsetWidth));
    const mid = left + el.offsetWidth / 2;
    // Where it would land: after every visible widget whose centre is left of ours.
    const others = ids.filter((x) => x !== d.id);
    const visible = others.filter((x) => slots[x]?.offsetWidth);
    const before = visible.filter((x) => slots[x].offsetLeft + slots[x].offsetWidth / 2 < mid).length;
    const next = [...others];
    const at = before === 0 ? (visible.length ? others.indexOf(visible[0]) : 0) : others.indexOf(visible[before - 1]) + 1;
    next.splice(at, 0, d.id);
    if (next.join() !== ids.join()) {
      order = next;
      await tick();
    }
    d.dx = left - el.offsetLeft;
  }

  function slotUp() {
    window.removeEventListener("pointermove", slotMove);
    window.removeEventListener("pointerup", slotUp);
    window.removeEventListener("pointercancel", slotUp);
    const d = wdrag;
    wdrag = null;
    if (!d?.active) return;
    // The release would otherwise click the tile under the pointer.
    const swallow = (e: Event) => {
      e.stopPropagation();
      e.preventDefault();
    };
    window.addEventListener("click", swallow, { capture: true, once: true });
    setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 80);
    settling = d.id;
    setTimeout(() => settling === d.id && (settling = null), 240);
    const final = order ?? shownIds;
    order = null;
    const all = store.s?.widgets ?? [];
    if (final.join() !== shownIds.join()) {
      updateSettings({
        widgets: [
          ...final.map((id) => ({ id, enabled: true })),
          ...all.filter((w) => !final.includes(w.id)),
        ],
      });
    }
  }

  // Slide the others with FLIP; the dragged one follows the pointer instead.
  function slide(node: Element, rects: { from: DOMRect; to: DOMRect }) {
    if (wdrag?.active && (node as HTMLElement).dataset.id === wdrag.id) return { duration: 0 };
    return flip(node, rects, { duration: 220, easing: cubicOut });
  }

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
  class:center={centered}
  class:dragging={drag?.active}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  oncontextmenu={menu}
>
  {#each ids as id (id)}
    {@const w = WIDGETS[id]}
    {@const dragging = wdrag?.active && wdrag.id === id}
    <div
      class="slot"
      class:grow={w.grow}
      class:lifted={dragging}
      class:settling={settling === id}
      role="group"
      data-id={id}
      bind:this={slots[id]}
      style:transform={dragging ? `translateX(${wdrag!.dx}px) scale(1.04)` : null}
      onpointerdown={(e) => slotDown(e, id)}
      animate:slide
    >
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
  /* Centered: everything sits together in the middle, like the Windows 11 taskbar. */
  .center {
    justify-content: center;
  }
  .center .slot.grow {
    flex: 0 1 auto;
  }
  .slot.lifted {
    position: relative;
    z-index: 2;
    cursor: grabbing;
    border-radius: 8px;
    background: var(--hover);
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.28);
    transition:
      background-color var(--t-fast) var(--ease-out),
      box-shadow var(--t-fast) var(--ease-out);
  }
  .slot.settling {
    transition: transform 200ms var(--ease-out);
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
