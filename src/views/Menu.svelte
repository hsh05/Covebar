<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { MENU, type MenuIcon, type MenuRow } from "../lib/menu";
  import { EXIT_MS } from "../lib/motion";

  let session = $state(0);
  let rows = $state<MenuRow[]>([]);
  let closing = $state(false);
  let focusIdx = $state(-1);

  listen<{ session: number; items: MenuRow[] }>("menu-open", (e) => {
    session = e.payload.session;
    rows = e.payload.items;
    closing = false;
    focusIdx = -1;
  });

  function close() {
    if (closing) return;
    closing = true;
    const s = session;
    setTimeout(() => invoke("close_menu", { session: s }), EXIT_MS);
  }

  function pick(r: MenuRow) {
    if (r.kind !== "item" || r.disabled || closing) return;
    emitTo("dock", "menu-pick", { session, id: r.id });
    close();
  }

  const enabled = $derived(
    rows.map((r, i) => (r.kind === "item" && !r.disabled ? i : -1)).filter((i) => i >= 0),
  );

  function step(dir: 1 | -1) {
    if (!enabled.length) return;
    const at = enabled.indexOf(focusIdx);
    focusIdx = enabled[(at + dir + enabled.length) % enabled.length] ?? enabled[0];
  }

  getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (!focused) close();
  });
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") close();
    else if (e.key === "ArrowDown") step(1);
    else if (e.key === "ArrowUp") step(-1);
    else if ((e.key === "Enter" || e.key === " ") && focusIdx >= 0) pick(rows[focusIdx]);
    else return;
    e.preventDefault();
  });

  const ICONS: Record<MenuIcon, string> = {
    settings: "M3 5h6M13 5h0M3 11h0M7 11h6M9 3v4M5 9v4",
    swap: "M5 13V3M2.5 5.5 5 3l2.5 2.5M11 3v10M8.5 10.5 11 13l2.5-2.5",
    activity: "M2 8h2.5L6 4l3 8 1.5-4H14",
    power: "M8 2.5v5M4.8 4.6a4.5 4.5 0 1 0 6.4 0",
    open: "M9 3h4v4M13 3 7.5 8.5M11 9.5V13H3V5h3.5",
    plus: "M8 3v10M3 8h10",
    close: "M4 4l8 8M12 4l-8 8",
    play: "M5 3.5v9l7.5-4.5z",
    pause: "M5.5 3.5v9M10.5 3.5v9",
    next: "M3.5 3.5l6 4.5-6 4.5zM12.5 3.5v9",
    prev: "M12.5 3.5l-6 4.5 6 4.5zM3.5 3.5v9",
    check: "M3.5 8.5l3 3 6-7",
    reset: "M3 8a5 5 0 1 0 1.5-3.6M3 2.5v3h3",
    timer: "M8 5.5V9l2 1.2M8 14.5a5.5 5.5 0 1 0 0-11 5.5 5.5 0 0 0 0 11zM6.5 1.5h3",
    clock: "M8 4.5V8l2.5 1.5M8 14a6 6 0 1 0 0-12 6 6 0 0 0 0 12z",
    flag: "M4 14.5V2.5M4 3h7.5L10 5.5 11.5 8H4",
    trash: "M2.5 4.5h11M6.5 4.5V2.5h3v2M4 4.5l.8 9h6.4l.8-9",
    sun: "M8 5.5a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5zM8 1.5V3M8 13v1.5M1.5 8H3M13 8h1.5M3.4 3.4l1 1M11.6 11.6l1 1M3.4 12.6l1-1M11.6 4.4l1-1",
    mic: "M8 2a2 2 0 0 1 2 2v4a2 2 0 0 1-4 0V4a2 2 0 0 1 2-2zM4.5 8a3.5 3.5 0 0 0 7 0M8 11.5V14",
    micOff: "M8 2a2 2 0 0 1 2 2v4a2 2 0 0 1-4 0V4a2 2 0 0 1 2-2zM4.5 8a3.5 3.5 0 0 0 7 0M8 11.5V14M2.5 2.5l11 11",
    speaker: "M2.5 6h2.5l3.5-3v10L5 10H2.5zM11 5.5a3.5 3.5 0 0 1 0 5",
    mute: "M2.5 6h2.5l3.5-3v10L5 10H2.5zM11 6.5l3 3M14 6.5l-3 3",
    sliders: "M4 2.5v11M8 2.5v11M12 2.5v11M2.5 10h3M6.5 5h3M10.5 9h3",
    hide: "M2 8s2.2-4 6-4 6 4 6 4-2.2 4-6 4-6-4-6-4zM8 6.5a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zM2.5 2.5l11 11",
    pin: "M6 2.5h4M7 2.5V6L4.5 9h7L9 6V2.5M8 9v5",
    unpin: "M6 2.5h4M7 2.5V6L4.5 9h7L9 6V2.5M8 9v5M2.5 2.5l11 11",
    shield: "M8 1.5l5 2v4.2c0 3-2.2 5.2-5 6.3-2.8-1.1-5-3.3-5-6.3V3.5z",
    minimize: "M3.5 11.5h9",
    restore: "M3 5.5h7.5V13H3zM5.5 5.5V3H13v7.5h-2.5",
    folder: "M2 4h4l1.5 1.5H14v7.5H2z",
  };
</script>

{#key session}
  <div
    class="surface menu"
    class:out={closing}
    style:--pad="{MENU.pad}px"
    style:--item="{MENU.item}px"
    style:--sep="{MENU.sep}px"
    style:--header="{MENU.header}px"
    role="menu"
    tabindex="-1"
  >
    {#each rows as r, i (i)}
      {#if r.kind === "header"}
        <div class="header">
          {#if r.image}<img src={r.image} alt="" width="20" height="20" />{/if}
          <span>{r.label}</span>
        </div>
      {:else if r.kind === "sep"}
        <div class="sep" role="separator"></div>
      {:else}
        <button
          class="item"
          class:danger={r.danger}
          class:focus={focusIdx === i}
          role="menuitem"
          disabled={r.disabled}
          style:--i={i}
          onclick={() => pick(r)}
          onpointerenter={() => (focusIdx = i)}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            {#if r.icon}<path d={ICONS[r.icon]} />{/if}
          </svg>
          <span class="label">{r.label}</span>
          {#if r.hint}<span class="hint">{r.hint}</span>{/if}
        </button>
      {/if}
    {/each}
  </div>
{/key}

<style>
  .menu {
    padding: var(--pad);
    display: flex;
    flex-direction: column;
    outline: none;
  }
  .header {
    flex: none;
    height: var(--header);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .header span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sep {
    flex: none;
    height: var(--sep);
    display: flex;
    align-items: center;
    padding: 0 6px;
  }
  .sep::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--line);
  }
  .item {
    flex: none;
    height: var(--item);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px;
    border-radius: 7px;
    text-align: left;
    transition: background-color 90ms var(--ease-out);
    /* Rows settle in one after another. */
    animation: row-in var(--t-med) var(--ease-out) both;
    animation-delay: calc(var(--i) * 12ms);
  }
  .item.focus {
    background: var(--hover);
  }
  .item:active {
    background: var(--pressed);
  }
  .item:disabled {
    opacity: 0.45;
  }
  .item.danger {
    color: var(--danger);
  }
  .item svg {
    flex: none;
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0.85;
  }
  .label {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hint {
    color: var(--text-faint);
    font-size: 12px;
  }
  @keyframes row-in {
    from {
      opacity: 0;
      transform: translateY(calc(var(--rise) * 0.4));
    }
  }
</style>
