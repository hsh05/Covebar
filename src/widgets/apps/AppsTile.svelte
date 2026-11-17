<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onDestroy } from "svelte";
  import { flip } from "svelte/animate";
  import { scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { iconUrl } from "../../lib/icons";
  import { showMenu, type MenuEntry } from "../../lib/menu";
  import { store } from "../../lib/settings.svelte";
  import { previewSize } from "../../lib/popups";
  import type { AppsSnapshot, DockApp, PreviewData } from "../../lib/types";
  import AppIcon from "./AppIcon.svelte";

  let snap = $state<AppsSnapshot>({ apps: [], active: 0 });
  invoke<AppsSnapshot>("get_apps").then((s) => (snap = s));
  const unlisten = listen<AppsSnapshot>("apps-changed", (e) => (snap = e.payload));
  // The backend hides the preview itself once the cursor is off the dock and preview.
  const unlistenHidden = listen("preview-hidden", () => (previewKey = null));
  onDestroy(() => {
    unlisten.then((f) => f());
    unlistenHidden.then((f) => f());
  });

  // A bit larger than the Windows taskbar (24 / 32) for an easier read.
  const iconPx = $derived(store.s?.size === "large" ? 36 : 28);

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
    const items: MenuEntry[] = [
      { header: app.name, image: await iconUrl(app.icon, 20) },
      { separator: true },
    ];
    const hwnds = app.windows.map((w) => w.hwnd);
    if (app.launch) {
      const target = app.launch;
      items.push({
        label: hwnds.length ? "Open new window" : "Open",
        icon: hwnds.length ? "plus" : "open",
        action: () => invoke("launch_app", { target }),
      });
      if (!target.startsWith("shell:"))
        items.push({ label: "Run as administrator", icon: "shield", action: () => invoke("run_as_admin", { target }) });
    }
    if (app.location) {
      const path = app.location;
      items.push({ label: "Open file location", icon: "folder", action: () => invoke("open_location", { path }) });
    }
    if (app.pinned && app.launch) {
      const lnk = app.launch;
      items.push({ label: "Unpin from the bar", icon: "unpin", action: () => invoke("unpin_app", { lnk }) });
    } else if (app.launch) {
      const launch = app.launch;
      items.push({ label: "Pin to the bar", icon: "pin", action: () => invoke("pin_app", { launch, name: app.name, location: app.location }) });
    }
    if (hwnds.length) {
      const allMin = app.windows.every((w) => w.minimized);
      items.push(
        { separator: true },
        allMin
          ? { label: hwnds.length > 1 ? "Restore all windows" : "Restore window", icon: "restore", action: () => invoke("show_windows", { hwnds, minimize: false }) }
          : { label: hwnds.length > 1 ? "Minimize all windows" : "Minimize window", icon: "minimize", action: () => invoke("show_windows", { hwnds, minimize: true }) },
        {
          label: hwnds.length > 1 ? `Close all ${hwnds.length} windows` : "Close window",
          icon: "close",
          danger: true,
          action: () => hwnds.forEach((hwnd) => invoke("close_window", { hwnd })),
        },
      );
    }
    await showMenu(items);
  }

  // Right-click Start: shortcuts to system tools, like Windows' own Start right-click menu.
  async function startMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    hidePreview();
    const run = (target: string) => () => invoke("launch_app", { target });
    await showMenu([
      { label: "Installed apps", icon: "open", action: run("ms-settings:appsfeatures") },
      { label: "Device Manager", icon: "sliders", action: run("devmgmt.msc") },
      { label: "Disk Management", icon: "folder", action: run("diskmgmt.msc") },
      { label: "Task Manager", icon: "activity", action: run("taskmgr.exe") },
      { label: "Settings", icon: "settings", action: run("ms-settings:") },
      { label: "File Explorer", icon: "folder", action: run("explorer.exe") },
      { label: "Run", icon: "open", action: run("shell:::{2559a1f3-21d7-11d4-bdaf-00c04f60b9f0}") },
    ]);
  }

  const isActive = (app: DockApp) => app.windows.some((w) => w.hwnd === snap.active);
</script>

<div class="apps">
  <button class="tile start" onclick={() => invoke("open_start")} oncontextmenu={startMenu} data-nodrag title="Start">
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="1.5" y="1.5" width="10" height="10" rx="0.6" />
      <rect x="12.5" y="1.5" width="10" height="10" rx="0.6" />
      <rect x="1.5" y="12.5" width="10" height="10" rx="0.6" />
      <rect x="12.5" y="12.5" width="10" height="10" rx="0.6" />
    </svg>
  </button>

  <div class="list">
    {#each snap.apps as app (app.key)}
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
        animate:flip={{ duration: 220, easing: cubicOut }}
        in:scale={{ start: 0.6, duration: 220, easing: cubicOut }}
        out:scale={{ start: 0.6, duration: 140 }}
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
    color: #0a84ff;
  }
  .start svg {
    width: calc(var(--icon) - 2px);
    height: calc(var(--icon) - 2px);
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
      width var(--t-med) var(--ease-out),
      background-color var(--t-med) var(--ease-out);
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
