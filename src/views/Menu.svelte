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
