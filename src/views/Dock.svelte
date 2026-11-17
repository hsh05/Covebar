<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { tick, untrack } from "svelte";
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import { getName } from "@tauri-apps/api/app";
  import { showMenu, type MenuEntry } from "../lib/menu";
  import { widgetMenu } from "../lib/widgetMenus";
  import { store, updateSettings } from "../lib/settings.svelte";
  import { WIDGETS } from "../lib/widgets";
  import type { Edge } from "../lib/types";

  let appName = $state("Covebar");
  getName().then((n) => (appName = n));

  const shownIds = $derived((store.s?.widgets ?? []).filter((w) => w.enabled && WIDGETS[w.id]).map((w) => w.id));
  const edge = $derived<Edge>(store.s?.edge ?? "bottom");
  const centered = $derived((store.s?.align ?? "center") === "center");
  const pill = $derived((store.s?.shape ?? "pill") === "pill");
  const glass = $derived(store.s?.material === "glass");

  // ---- the visible bar: measured here, drawn by .bg, and the only part of the window that
  // takes clicks (everything else passes through to the windows below)
  type Box = { x: number; y: number; w: number; h: number; r: number };
  let barEl = $state<HTMLElement>();
  let rowEl = $state<HTMLElement>();
  let bg = $state<Box>({ x: 0, y: 0, w: 0, h: 0, r: 0 });
  let ready = $state(false); // transitions on only after the first layout
  let morphing = $state(false); // switching shape: keep the bar opaque until the backdrop settles
  let measured = false;
  let regionTimer: ReturnType<typeof setTimeout> | undefined;

  function target(): Box | null {
    if (!barEl || !rowEl) return null;
    const W = barEl.clientWidth;
    const H = barEl.clientHeight;
    const h = rowEl.offsetHeight;
    if (pill) {
      const r = rowEl.getBoundingClientRect();
      return { x: r.left, y: edge === "bottom" ? H - 4 - h : 4, w: r.width, h, r: h / 2 };
    }
    return { x: 0, y: edge === "bottom" ? H - h : 0, w: W, h, r: 0 };
  }

  function sendRegion(b: Box, pad: number) {
    if (drag?.active) return;
    const W = barEl?.clientWidth ?? 0;
    const H = barEl?.clientHeight ?? 0;
    const x = Math.max(0, b.x - pad);
    const y = Math.max(0, b.y - pad);
    invoke("set_dock_region", {
      x,
      y,
      w: Math.min(W, b.x + b.w + pad) - x,
      h: Math.min(H, b.y + b.h + pad) - y,
      // Glass: round the region so the blur behind takes the pill's shape.
      radius: glass && b.r ? b.r + pad : 0,
      full: false,
    });
  }

  function update() {
    const t = target();
    if (!t) return;
    const prev = bg;
    bg = t;
    const pad = pill && !glass ? 4 : 0;
    if (!measured) {
      measured = true;
      sendRegion(t, pad);
      requestAnimationFrame(() => requestAnimationFrame(() => (ready = true)));
      return;
    }
    // While it animates, cover both the old and the new shape; then shrink to the new one.
    const x = Math.min(prev.x, t.x);
    const y = Math.min(prev.y, t.y);
    const union = { x, y, w: Math.max(prev.x + prev.w, t.x + t.w) - x, h: Math.max(prev.y + prev.h, t.y + t.h) - y, r: 0 };
    sendRegion(union, pad);
    clearTimeout(regionTimer);
    regionTimer = setTimeout(() => sendRegion(bg, pad), 480);
  }

  let lastPill: boolean | undefined;
  $effect(() => {
    // Re-measure when the shape, edge, alignment, size or widgets change (after the DOM updates).
    void [pill, glass, edge, centered, store.s?.size, ids.length];
    if (lastPill !== undefined && lastPill !== pill) {
      morphing = true;
      setTimeout(() => (morphing = false), 480);
    }
    lastPill = pill;
    untrack(update);
  });
  $effect(() => {
    if (!rowEl) return;
    const ro = new ResizeObserver(() => untrack(update));
    ro.observe(rowEl);
    const onResize = () => untrack(update);
    window.addEventListener("resize", onResize);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", onResize);
    };
  });
  // Dragging the bar to the other edge shows a hint over the whole window.
  $effect(() => {
    if (drag?.active) invoke("set_dock_region", { x: 0, y: 0, w: 0, h: 0, full: true });
    else untrack(update);
  });

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

  // Right-click a widget: its own actions, then hide / customize. Widgets without a menu
  // (the apps strip's empty space) fall through to the bar menu.
  async function slotMenu(e: MouseEvent, id: string) {
    const def = WIDGETS[id];
    if (!def || def.grow) return;
    e.preventDefault();
    e.stopPropagation();
    const openCard = () => slots[id]?.querySelector<HTMLElement>(".tile")?.click();
    const own = await widgetMenu(id, openCard);
    const hide = () =>
      updateSettings({ widgets: (store.s?.widgets ?? []).map((w) => (w.id === id ? { ...w, enabled: false } : w)) });
    const entries: MenuEntry[] = [
      ...own,
      ...(own.length ? [{ separator: true } as const] : []),
      { label: `Hide ${def.name.toLowerCase()} from the bar`, icon: "hide", action: hide },
      { label: "Customize widgets…", icon: "settings", action: () => invoke("open_settings") },
    ];
    await showMenu(entries);
  }

  async function menu(e: MouseEvent) {
    e.preventDefault();
    const other: Edge = edge === "bottom" ? "top" : "bottom";
    const autoHide = store.s?.autoHide ?? false;
    await showMenu([
      { label: "Task Manager", icon: "activity", action: () => invoke("launch_app", { target: "taskmgr.exe" }) },
      { label: `${appName} settings`, icon: "settings", action: () => invoke("open_settings") },
      { label: `Move bar to the ${other}`, icon: "swap", action: () => updateSettings({ edge: other }) },
      {
        label: "Automatically hide the bar",
        icon: autoHide ? "check" : undefined,
        action: () => updateSettings({ autoHide: !autoHide }),
      },
      { separator: true },
      { label: `Quit ${appName}`, icon: "power", danger: true, action: () => invoke("quit") },
    ]);
  }
</script>

<main
  bind:this={barEl}
  class="bar"
  class:top={edge === "top"}
  class:center={centered}
  class:pill
  class:glass
  class:dragging={drag?.active}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  oncontextmenu={menu}
>
  <div
    class="bg"
    class:pill
    class:morphing
    class:ready
    style:left="{bg.x}px"
    style:top="{bg.y}px"
    style:width="{bg.w}px"
    style:height="{bg.h}px"
    style:border-radius="{bg.r}px"
  ></div>
  <div class="row" class:ready bind:this={rowEl}>
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
      oncontextmenu={(e) => slotMenu(e, id)}
      animate:slide
    >
      <w.tile />
    </div>
  {/each}
  </div>

  {#if drag?.active}
    <div class="hint">
      {drag.target === edge ? "Drag towards the other edge" : `Release to move the bar to the ${drag.target}`}
    </div>
  {/if}
</main>

<style>
  .bar {
    /* Full bar <-> pill: long and gentle, settling softly at the end. */
    --morph: 440ms;
    --ease-morph: cubic-bezier(0.32, 0.72, 0, 1);
    position: relative;
    height: 100vh;
  }
  /* The visible bar. It's drawn as its own layer so it can morph between the full-width
     strip and the pill; only this area takes clicks (the backend clips the window to it). */
  .bg {
    position: absolute;
    background: var(--tint);
    border: 1px solid transparent;
    border-top-color: var(--line);
  }
  .top .bg {
    border-top-color: transparent;
    border-bottom-color: var(--line);
  }
  .bg.pill,
  .bg.morphing {
    background: var(--pill);
  }
  .bg.pill {
    border-color: var(--surface-line);
    box-shadow: 0 2px 6px rgb(0 0 0 / 0.22);
  }
  /* Liquid glass, like a thick clear lens: almost no tint, the blurred desktop shows
     through, and the light lives on the rim. It is brightest where it faces the light (top
     left) and where light bounces back out (bottom right), with a soft glow just inside the
     edge that gives the glass its thickness. */
  .glass .bg,
  .glass .bg.pill,
  .glass .bg.morphing {
    background: var(--glass-tint);
    border-color: transparent;
    box-shadow:
      inset 2px 3px 2px -2px rgb(255 255 255 / 0.35),
      inset -2px -3px 2px -2px rgb(255 255 255 / 0.2),
      inset 0 0 12px rgb(255 255 255 / 0.1),
      inset 0 -12px 16px -12px rgb(255 255 255 / 0.2);
  }
  /* The pill has no system blur behind it (Windows can't shape one to a pill), so the
     glass gets a little more body of its own. */
  .glass .bg.pill {
    background: var(--glass-pill);
  }
  /* The rim: a gradient ring, bright at the corners that catch the light. */
  .glass .bg::before {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    padding: 1px;
    pointer-events: none;
    background: linear-gradient(
      135deg,
      rgb(255 255 255 / 0.5),
      rgb(255 255 255 / 0.1) 28%,
      rgb(255 255 255 / 0.03) 52%,
      rgb(255 255 255 / 0.1) 78%,
      rgb(255 255 255 / 0.4)
    );
    -webkit-mask:
      linear-gradient(#000 0 0) content-box,
      linear-gradient(#000 0 0);
    mask:
      linear-gradient(#000 0 0) content-box,
      linear-gradient(#000 0 0);
    -webkit-mask-composite: xor;
    mask-composite: exclude;
  }
  /* The full bar only has a rim along its open side. */
  .glass .bg:not(.pill)::before {
    padding: 1px 0 0;
    background: linear-gradient(90deg, rgb(255 255 255 / 0.1), rgb(255 255 255 / 0.32) 50%, rgb(255 255 255 / 0.1));
  }
  .glass.top .bg:not(.pill)::before {
    padding: 0 0 1px;
  }
  /* Light pooling inside the glass, top left, and a faint caustic bottom right. */
  .glass .bg::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    pointer-events: none;
    background:
      radial-gradient(70% 140% at 12% -10%, rgb(255 255 255 / 0.16), transparent 55%),
      radial-gradient(50% 120% at 92% 115%, rgb(255 255 255 / 0.1), transparent 60%);
  }
  /* Hovered and active tiles become little glass bubbles of their own. */
  .glass .row :global(.tile) {
    transition:
      background-color var(--t-fast) var(--ease-out),
      box-shadow var(--t-fast) var(--ease-out),
      transform var(--t-fast) var(--ease-out);
  }
  .glass .row :global(.tile:hover),
  .glass .row :global(.tile.active) {
    background-color: rgb(255 255 255 / 0.08);
    background-image: radial-gradient(130% 110% at 30% 0%, rgb(255 255 255 / 0.16), transparent 62%);
    box-shadow:
      inset 1.5px 2px 1px -1px rgb(255 255 255 / 0.7),
      inset -1.5px -2px 1px -1px rgb(255 255 255 / 0.35),
      inset 0 0 0 1px rgb(255 255 255 / 0.12),
      inset 0 0 8px rgb(255 255 255 / 0.08),
      0 2px 8px rgb(0 0 0 / 0.14);
  }
  .glass .row :global(.tile:active) {
    background-color: rgb(255 255 255 / 0.16);
  }
  :global([data-theme="dark"]) .glass .row :global(.tile) {
    text-shadow: 0 1px 3px rgb(0 0 0 / 0.35);
  }
  .bg.ready {
    transition:
      left var(--morph) var(--ease-morph),
      top var(--morph) var(--ease-morph),
      width var(--morph) var(--ease-morph),
      height var(--morph) var(--ease-morph),
      border-radius var(--morph) var(--ease-morph),
      border-color var(--morph) var(--ease-morph),
      box-shadow var(--morph) var(--ease-morph),
      background-color 300ms var(--ease-out);
  }

  .row {
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: 0;
    height: var(--bar);
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .top .row {
    top: 0;
    bottom: auto;
  }
  .row.ready {
    transition:
      top var(--morph) var(--ease-morph),
      bottom var(--morph) var(--ease-morph),
      padding var(--morph) var(--ease-morph);
  }
  .center .row {
    justify-content: center;
  }
  /* Pill: the row shrinks to its widgets and floats a little off the edge. */
  .pill .row {
    right: auto;
    bottom: 4px;
    max-width: calc(100% - 16px);
    padding: 0 4px;
  }
  .pill.top .row {
    top: 4px;
  }
  .pill.center .row {
    left: 50%;
    transform: translateX(-50%);
  }
  /* Tiles at the ends follow the pill's curve. */
  .pill .row > :global(.slot:first-child .tile:first-child),
  .pill .row > :global(.slot:last-child .tile:last-child) {
    border-radius: calc((var(--bar) - 8px) / 2);
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
  /* Centered or pill: everything sits together, like the Windows 11 taskbar. */
  .center .slot.grow,
  .pill .slot.grow {
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
