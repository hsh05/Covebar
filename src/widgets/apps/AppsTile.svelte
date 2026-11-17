<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
  import { onDestroy } from "svelte";
  import { store } from "../../lib/settings.svelte";
  import { previewSize } from "../../lib/popups";
  import type { AppsSnapshot, DockApp, PreviewData } from "../../lib/types";
  import AppIcon from "./AppIcon.svelte";

  let snap = $state<AppsSnapshot>({ apps: [], active: 0 });
  invoke<AppsSnapshot>("get_apps").then((s) => (snap = s));
  const unlisten = listen<AppsSnapshot>("apps-changed", (e) => (snap = e.payload));
  onDestroy(() => unlisten.then((f) => f()));

  const iconPx = $derived(store.s?.size === "large" ? 36 : 26);

  // ---- hover previews
  let hoverTimer: ReturnType<typeof setTimeout> | undefined;
  let previewKey: string | null = null;

  function showPreview(app: DockApp, el: HTMLElement) {
    if (!app.windows.length) return;
    const r = el.getBoundingClientRect();
    const data: PreviewData = {
      appKey: app.key,
      name: app.name,
      icon: app.icon,
      windows: app.windows.map((w) => ({ hwnd: w.hwnd, title: w.title })),
    };
    const size = previewSize(app.windows.length);
    previewKey = app.key;
    invoke("show_preview", { data, anchorX: r.left, anchorWidth: r.width, ...size });
  }

  function enter(app: DockApp, e: PointerEvent) {
    const el = e.currentTarget as HTMLElement;
    clearTimeout(hoverTimer);
    if (!app.windows.length) {
      if (previewKey) invoke("preview_release");
      return;
    }
    if (previewKey === app.key) {
      invoke("preview_hold");
      return;
    }
    // Already previewing another app: switch at once; otherwise wait a moment.
    hoverTimer = setTimeout(() => showPreview(app, el), previewKey ? 0 : 420);
  }

  function leave() {
    clearTimeout(hoverTimer);
    if (previewKey) invoke("preview_release");
    setTimeout(() => (previewKey = null), 340);
  }

  function hidePreview() {
    clearTimeout(hoverTimer);
    previewKey = null;
    invoke("hide_preview");
  }

  // ---- clicks
  function click(app: DockApp) {
    hidePreview();
    if (app.windows.length) invoke("click_app", { hwnds: app.windows.map((w) => w.hwnd) });
    else if (app.launch) invoke("launch_app", { target: app.launch });
  }

  function aux(app: DockApp, e: MouseEvent) {
    if (e.button === 1 && app.launch) {
      e.preventDefault();
      invoke("launch_app", { target: app.launch });
    }
  }

  async function contextMenu(app: DockApp, e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    hidePreview();
    const items: (MenuItem | PredefinedMenuItem)[] = [
      await MenuItem.new({ text: app.name, enabled: false }),
      await PredefinedMenuItem.new({ item: "Separator" }),
    ];
    if (app.launch) {
      items.push(
        await MenuItem.new({
          text: app.windows.length ? "Open new window" : "Open",
          action: () => invoke("launch_app", { target: app.launch }),
        }),
      );
    }
    if (app.windows.length) {
      items.push(
        await MenuItem.new({
          text: app.windows.length > 1 ? `Close all ${app.windows.length} windows` : "Close window",
          action: () => app.windows.forEach((w) => invoke("close_window", { hwnd: w.hwnd })),
        }),
      );
    }
    const menu = await Menu.new({ items });
    await menu.popup();
  }

  const isActive = (app: DockApp) => app.windows.some((w) => w.hwnd === snap.active);
</script>

<div class="apps">
  <button class="tile start" onclick={() => invoke("open_start")} data-nodrag title="Start">
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="6" cy="6" r="2.2" /><circle cx="12" cy="6" r="2.2" /><circle cx="18" cy="6" r="2.2" />
      <circle cx="6" cy="12" r="2.2" /><circle cx="12" cy="12" r="2.2" /><circle cx="18" cy="12" r="2.2" />
      <circle cx="6" cy="18" r="2.2" /><circle cx="12" cy="18" r="2.2" /><circle cx="18" cy="18" r="2.2" />
    </svg>
  </button>

  <div class="list">
    {#each snap.apps as app, i (app.key)}
      {#if i > 0 && !app.pinned && snap.apps[i - 1].pinned}
        <span class="divider" aria-hidden="true"></span>
      {/if}
      <button
        class="tile app"
        class:running={app.windows.length > 0}
        class:active={isActive(app)}
        data-nodrag
        title={app.windows.length === 1 ? app.windows[0].title : app.name}
        onclick={() => click(app)}
        onauxclick={(e) => aux(app, e)}
        onmousedown={(e) => e.button === 1 && e.preventDefault()}
        oncontextmenu={(e) => contextMenu(app, e)}
        onpointerenter={(e) => enter(app, e)}
        onpointerleave={leave}
      >
        <AppIcon {app} size={iconPx} />
        {#if app.windows.length}
          <span class="dot" class:many={app.windows.length > 1}></span>
        {/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .apps {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 100%;
    min-width: 0;
  }
  .start {
    width: calc(var(--bar) - 8px);
    flex: none;
    color: var(--accent);
  }
  .start svg {
    width: calc(var(--icon) - 6px);
    height: calc(var(--icon) - 6px);
    fill: currentColor;
  }
  .list {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
  }
  .divider {
    flex: none;
    width: 1px;
    height: calc(var(--bar) * 0.42);
    background: var(--line);
    margin: 0 4px;
  }
  .app {
    position: relative;
    flex: none;
    width: calc(var(--bar) - 4px);
  }
  .app.active {
    background: var(--raised);
  }
  .dot {
    position: absolute;
    left: 50%;
    bottom: 2px;
    width: 5px;
    height: 3px;
    border-radius: 2px;
    background: var(--text-faint);
    transform: translateX(-50%);
    transition:
      width 160ms ease,
      background-color 160ms ease;
  }
  :global([data-edge="top"]) .dot {
    bottom: auto;
    top: 2px;
  }
  .dot.many {
    width: 9px;
  }
  .active .dot {
    width: 16px;
    background: var(--accent);
  }
</style>
