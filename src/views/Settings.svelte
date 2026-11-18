<script lang="ts">
  import { getName, getVersion } from "@tauri-apps/api/app";
  import { invoke } from "@tauri-apps/api/core";
  import { store, updateSettings } from "../lib/settings.svelte";
  import { WIDGETS } from "../lib/widgets";
  import type { Settings, ShortcutName, WidgetEntry } from "../lib/types";
  import { hotkeyStatus } from "../lib/live.svelte";
  import { presetLabel } from "../lib/format";
  import ShortcutRecorder from "./ShortcutRecorder.svelte";

  let appName = $state("Covebar");
  let version = $state("");
  getName().then((n) => (appName = n));
  getVersion().then((v) => (version = v));

  const sections = ["General", "Appearance", "Widgets", "Shortcuts", "Clock", "Timer", "About"] as const;
  let section = $state<(typeof sections)[number]>("General");
  let error = $state("");

  async function set<K extends keyof Settings>(key: K, value: Settings[K]) {
    error = "";
    try {
      await updateSettings({ [key]: value } as Partial<Settings>);
    } catch (e) {
      error = String(e);
    }
  }

  // ---- backup: export to a JSON file (saved to Downloads) and import one back
  let backupNote = $state("");
  let importInput = $state<HTMLInputElement>();
  function exportSettings() {
    const data = { app: appName, version, exportedAt: new Date().toISOString(), settings: $state.snapshot(s) };
    const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: "application/json" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `${appName.toLowerCase()}-settings.json`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    backupNote = "Saved to your Downloads folder.";
  }
  async function importSettings(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    error = "";
    backupNote = "";
    try {
      const data = JSON.parse(await file.text());
      const imported = data && typeof data === "object" && "settings" in data ? data.settings : data;
      if (!imported || typeof imported !== "object" || !Array.isArray(imported.widgets)) {
        throw new Error("That file isn't a Covebar settings backup.");
      }
      await updateSettings(imported as Partial<Settings>);
      backupNote = `Imported settings from ${file.name}.`;
    } catch (err) {
      error = err instanceof SyntaxError ? "That file isn't valid JSON." : err instanceof Error ? err.message : String(err);
    }
  }

  // ---- shortcuts and timer presets
  const keys = hotkeyStatus();
  const DEFAULT_KEYS: Record<ShortcutName, string> = {
    micMute: "Win+Alt+M",
    speakerMute: "Win+Alt+S",
    focusBar: "Win+Alt+Q",
  };
  const SHORTCUTS: [ShortcutName, string, string][] = [
    ["micMute", "Mute microphone", "Turns your microphone on or off"],
    ["speakerMute", "Mute speakers", "Turns the sound output on or off"],
    ["focusBar", "Focus the bar", "Then use the arrow keys, Enter to open, the Menu key for options and Esc to leave"],
  ];
  function setKey(k: ShortcutName | "winNumbers", v: string | boolean) {
    set("hotkeys", { ...s.hotkeys, [k]: v });
  }

  let newPreset = $state("");
  function addPreset() {
    const min = parseFloat(newPreset.replace(",", "."));
    if (!(min > 0 && min <= 1440)) return;
    const sec = Math.round(min * 60);
    newPreset = "";
    if (!s.timerPresets.includes(sec)) set("timerPresets", [...s.timerPresets, sec].sort((a, b) => a - b));
  }

  // ---- widgets: drag between "In the bar" and "Hidden", or reorder within a list
  const s = $derived(store.s!);
  const shown = $derived(s.widgets.filter((w) => w.enabled && WIDGETS[w.id]));
  const hidden = $derived(s.widgets.filter((w) => !w.enabled && WIDGETS[w.id]));

  let dragId = $state<string | null>(null);
  let dropAt = $state<{ list: "shown" | "hidden"; index: number } | null>(null);

  function overItem(list: "shown" | "hidden", index: number, e: DragEvent) {
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    dropAt = { list, index: e.clientY > r.top + r.height / 2 ? index + 1 : index };
  }
  function overList(list: "shown" | "hidden", len: number, e: DragEvent) {
    e.preventDefault();
    if (e.target === e.currentTarget) dropAt = { list, index: len };
  }
  function drop(e: DragEvent) {
    e.preventDefault();
    if (!dragId || !dropAt) return finish();
    const id = dragId;
    const lists = {
      shown: shown.map((w) => w.id).filter((x) => x !== id),
      hidden: hidden.map((w) => w.id).filter((x) => x !== id),
    };
    const src = shown.findIndex((w) => w.id === id);
    let idx = dropAt.index;
    if (dropAt.list === "shown" && src !== -1 && src < idx) idx--;
    const hsrc = hidden.findIndex((w) => w.id === id);
    if (dropAt.list === "hidden" && hsrc !== -1 && hsrc < idx) idx--;
    lists[dropAt.list].splice(idx, 0, id);
    const widgets: WidgetEntry[] = [
      ...lists.shown.map((id) => ({ id, enabled: true })),
      ...lists.hidden.map((id) => ({ id, enabled: false })),
    ];
    finish();
    set("widgets", widgets);
  }
  function finish() {
    dragId = null;
    dropAt = null;
  }
  function toggle(id: string) {
    set(
      "widgets",
      s.widgets.map((w) => (w.id === id ? { ...w, enabled: !w.enabled } : w)),
    );
  }
</script>

{#snippet toggleRow(label: string, hint: string, key: "autostart" | "hideTaskbar" | "autoHide" | "clock24h" | "clockSeconds" | "weekStartsMonday")}
  <label class="row">
    <span class="text"><span class="label">{label}</span><span class="hint">{hint}</span></span>
    <input type="checkbox" class="switch" checked={s[key]} onchange={(e) => set(key, e.currentTarget.checked)} />
  </label>
{/snippet}

{#snippet choice<T extends string>(label: string, value: T, options: [T, string][], onpick: (v: T) => void)}
  <div class="row">
    <span class="text"><span class="label">{label}</span></span>
    <div class="seg" role="radiogroup" aria-label={label}>
      {#each options as [v, text]}
        <button role="radio" aria-checked={value === v} class:on={value === v} onclick={() => onpick(v)}>{text}</button>
      {/each}
    </div>
  </div>
{/snippet}

{#snippet widgetList(list: "shown" | "hidden", items: WidgetEntry[])}
  <ul
    class="wlist"
    class:target={dropAt?.list === list}
    ondragover={(e) => overList(list, items.length, e)}
    ondrop={drop}
    ondragleave={(e) => e.target === e.currentTarget && (dropAt = null)}
  >
    {#each items as w, i (w.id)}
      <li
        class="witem"
        class:dragging={dragId === w.id}
        class:before={dropAt?.list === list && dropAt.index === i}
        class:after={dropAt?.list === list && dropAt.index === i + 1 && i === items.length - 1}
        draggable="true"
        ondragstart={(e) => {
          dragId = w.id;
          e.dataTransfer?.setData("text/plain", w.id);
        }}
        ondragend={finish}
        ondragover={(e) => overItem(list, i, e)}
      >
        <svg class="grip" viewBox="0 0 10 16" aria-hidden="true">
          <circle cx="3" cy="3" r="1.3" /><circle cx="7" cy="3" r="1.3" /><circle cx="3" cy="8" r="1.3" />
          <circle cx="7" cy="8" r="1.3" /><circle cx="3" cy="13" r="1.3" /><circle cx="7" cy="13" r="1.3" />
        </svg>
        <span class="text">
          <span class="label">{WIDGETS[w.id].name}</span>
          <span class="hint">{WIDGETS[w.id].description}</span>
        </span>
        <input type="checkbox" class="switch" checked={w.enabled} onchange={() => toggle(w.id)} aria-label="Show {WIDGETS[w.id].name}" />
      </li>
    {:else}
      <li class="empty">Drop widgets here</li>
    {/each}
  </ul>
{/snippet}

<div class="settings">
  <nav>
    <div class="brand">
      <img src="/brand.svg" alt="" width="28" height="28" />
      <span>{appName}</span>
    </div>
    {#each sections as name}
      <button class:on={section === name} onclick={() => (section = name)}>{name}</button>
    {/each}
  </nav>

  <section class="content">
    <h1>{section}</h1>
    {#if error}<p class="error">{error}</p>{/if}

    {#if section === "General"}
      <div class="group">
        {@render toggleRow("Start with Windows", `Launch ${appName} when you sign in`, "autostart")}
        {@render toggleRow(
          "Hide the Windows taskbar",
          `The taskbar comes back when ${appName} quits, even after a crash`,
          "hideTaskbar",
        )}
        {@render toggleRow(
          "Automatically hide the bar",
          "It slides away and comes back when the pointer touches the screen edge",
          "autoHide",
        )}
      </div>
      <div class="group">
        {@render choice("Position", s.edge, [["bottom", "Bottom"], ["top", "Top"]], (v) => set("edge", v))}
      </div>
      <p class="note">You can also drag the bar's empty space to the top or bottom of the screen.</p>
      <div class="group">
        <div class="row">
          <div class="text">
            <span class="label">Back up settings</span>
            <span class="hint">Move your bar, widgets and shortcuts to another PC</span>
          </div>
          <div class="backup">
            <button class="plain" onclick={exportSettings}>Export…</button>
            <button class="plain" onclick={() => importInput?.click()}>Import…</button>
            <input bind:this={importInput} type="file" accept=".json,application/json" hidden onchange={importSettings} />
          </div>
        </div>
      </div>
      {#if backupNote}<p class="note ok">{backupNote}</p>{/if}
    {:else if section === "Appearance"}
      <div class="group">
        {@render choice("Theme", s.theme, [["system", "System"], ["dark", "Dark"], ["light", "Light"]], (v) => set("theme", v))}
        {@render choice("Bar", s.shape, [["pill", "Pill"], ["full", "Full width"]], (v) => set("shape", v))}
        {@render choice("Widgets", s.align, [["center", "Centered"], ["left", "Left"]], (v) => set("align", v))}
        {@render choice("Size", s.size, [["small", "Small"], ["large", "Large"]], (v) => set("size", v))}
        {@render choice("Background", s.material, [["acrylic", "Acrylic"], ["mica", "Mica"], ["glass", "Liquid glass"]], (v) => set("material", v))}
      </div>
      <p class="note">
        Mica needs Windows 11; on Windows 10 the bar uses Acrylic. The background applies to the full-width bar;
        the pill has its own solid look.
      </p>
    {:else if section === "Widgets"}
      <p class="note top">Drag widgets to reorder them or move them between the lists. You can also drag them around on the bar itself.</p>
      <h2>In the bar</h2>
      {@render widgetList("shown", shown)}
      <h2>Hidden</h2>
      {@render widgetList("hidden", hidden)}
    {:else if section === "Shortcuts"}
      <div class="group">
        {#each SHORTCUTS as [key, label, hint] (key)}
          <div class="row">
            <span class="text"><span class="label">{label}</span><span class="hint">{hint}</span></span>
            <div>
              <ShortcutRecorder
                value={s.hotkeys[key]}
                fallback={DEFAULT_KEYS[key]}
                status={keys.v[key]}
                onchange={(v) => setKey(key, v)}
              />
            </div>
          </div>
        {/each}
      </div>
      <div class="group">
        <label class="row">
          <span class="text">
            <span class="label">Win+1 to Win+9 open your apps</span>
            <span class="hint">Like the Windows taskbar: Win+1 is the first app on the bar, Win+0 the tenth</span>
          </span>
          <input
            type="checkbox"
            class="switch"
            checked={s.hotkeys.winNumbers}
            onchange={(e) => setKey("winNumbers", e.currentTarget.checked)}
          />
        </label>
      </div>
      <p class="note">Shortcuts work everywhere, even while a game or another app is in front.</p>
    {:else if section === "Timer"}
      <div class="group">
        <div class="row presets-row">
          <span class="text">
            <span class="label">Presets</span>
            <span class="hint">One-click durations on the timer card</span>
          </span>
        </div>
        <div class="presets">
          {#each s.timerPresets as sec (sec)}
            <span class="preset">
              {presetLabel(sec)}
              <button
                aria-label="Remove {presetLabel(sec)}"
                onclick={() => set("timerPresets", s.timerPresets.filter((x) => x !== sec))}>×</button
              >
            </span>
          {/each}
          {#if s.timerPresets.length < 8}
            <form class="add" onsubmit={(e) => (e.preventDefault(), addPreset())}>
              <input type="text" inputmode="decimal" placeholder="Minutes" bind:value={newPreset} />
              <button type="submit" disabled={!newPreset.trim()}>Add</button>
            </form>
          {/if}
        </div>
      </div>
      <p class="note">When a timer ends, a popup appears above the timer and an alarm rings until you dismiss it.</p>
    {:else if section === "Clock"}
      <div class="group">
        {@render toggleRow("24-hour time", "Show 14:05 instead of 2:05 PM", "clock24h")}
        {@render toggleRow("Show seconds", "Show seconds in the bar", "clockSeconds")}
        {@render toggleRow("Week starts on Monday", "Used by the calendar card", "weekStartsMonday")}
      </div>
    {:else if section === "About"}
      <div class="group about">
        <p><strong>{appName}</strong> {version}</p>
        <p class="hint">A dock and widget bar for Windows. MIT licensed.</p>
        <p class="hint">
          If the Windows taskbar ever stays hidden, run <code>{appName.toLowerCase()}.exe --restore-taskbar</code>.
        </p>
      </div>
      <div class="about-actions">
        <button onclick={() => invoke("show_tour")}>Show the tour</button>
        <button onclick={() => invoke("open_log_folder")}>Open log folder</button>
        <button class="danger" onclick={() => invoke("quit")}>Quit {appName}</button>
      </div>
    {/if}
  </section>
</div>

<style>
  .backup {
    display: flex;
    gap: 6px;
  }
  .plain {
    padding: 6px 12px;
    border-radius: 6px;
    border: 1px solid var(--line);
    transition: background-color var(--t-fast) var(--ease-out);
  }
  .plain:hover {
    background: var(--hover);
  }
  .note.ok {
    color: var(--accent);
  }
  .about-actions {
    display: flex;
    gap: 8px;
  }
  .settings {
    display: flex;
    height: 100vh;
    user-select: none;
  }
  nav {
    width: 200px;
    flex: none;
    padding: 14px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 10px 16px;
    font: 600 15px var(--font-display);
  }
  nav button {
    text-align: left;
    padding: 8px 12px;
    border-radius: 6px;
    position: relative;
  }
  nav button:hover {
    background: var(--hover);
  }
  nav button.on {
    background: var(--raised);
    font-weight: 600;
  }
  nav button.on::before {
    content: "";
    position: absolute;
    left: 0;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 2px;
    background: var(--accent);
  }
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 18px 28px 28px;
    background: var(--card-tint);
    border-top-left-radius: 8px;
    border-left: 1px solid var(--line);
    border-top: 1px solid var(--line);
  }
  h1 {
    font: 600 26px var(--font-display);
    margin: 4px 0 18px;
  }
  h2 {
    font-size: 13px;
    font-weight: 600;
    margin: 18px 0 8px;
  }
  .group {
    background: var(--raised);
    border: 1px solid var(--line);
    border-radius: 8px;
    margin-bottom: 10px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 16px;
    min-height: 60px;
  }
  .row + .row {
    border-top: 1px solid var(--line);
  }
  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .hint {
    color: var(--text-dim);
    font-size: 12px;
  }
  .note {
    color: var(--text-dim);
    font-size: 12px;
    margin: 6px 2px 0;
  }
  .note.top {
    margin: 0 0 4px;
  }
  .error {
    color: var(--danger);
  }
  .switch {
    appearance: none;
    flex: none;
    width: 40px;
    height: 20px;
    border-radius: 10px;
    border: 1px solid var(--text-dim);
    position: relative;
    transition: background-color 120ms;
    margin: 0;
  }
  .switch::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--text-dim);
    transition: transform 140ms ease;
  }
  .switch:checked {
    background: var(--accent);
    border-color: var(--accent);
  }
  .switch:checked::after {
    background: var(--accent-text);
    transform: translateX(20px);
  }
  .seg {
    display: flex;
    background: var(--pressed);
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 2px;
  }
  .seg button {
    padding: 5px 14px;
    border-radius: 5px;
  }
  .seg button:hover:not(.on) {
    background: var(--hover);
  }
  .seg button.on {
    background: var(--accent);
    color: var(--accent-text);
    font-weight: 600;
  }
  .wlist {
    list-style: none;
    margin: 0;
    padding: 4px;
    min-height: 56px;
    border: 1px dashed var(--line);
    border-radius: 8px;
    transition: border-color 120ms;
  }
  .wlist.target {
    border-color: var(--accent);
  }
  .witem {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 6px;
    background: var(--raised);
    margin: 3px 0;
    cursor: grab;
    position: relative;
  }
  .witem.dragging {
    opacity: 0.4;
  }
  .witem.before::before,
  .witem.after::after {
    content: "";
    position: absolute;
    left: 6px;
    right: 6px;
    height: 2px;
    border-radius: 1px;
    background: var(--accent);
  }
  .witem.before::before {
    top: -4px;
  }
  .witem.after::after {
    bottom: -4px;
  }
  .grip {
    width: 10px;
    height: 16px;
    fill: var(--text-faint);
    flex: none;
  }
  .empty {
    padding: 14px;
    text-align: center;
    color: var(--text-faint);
  }
  .presets-row {
    min-height: 0;
    padding-bottom: 4px;
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 4px 16px 14px;
  }
  .preset {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 28px;
    padding: 0 4px 0 11px;
    border-radius: 14px;
    background: var(--pressed);
    border: 1px solid var(--line);
    font-size: 12px;
  }
  .preset button {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    color: var(--text-dim);
  }
  .preset button:hover {
    background: var(--hover);
    color: var(--danger);
  }
  .add {
    display: inline-flex;
    gap: 4px;
  }
  .add input {
    width: 80px;
    height: 28px;
    padding: 0 10px;
    border-radius: 14px;
    border: 1px solid var(--line);
    background: var(--pressed);
    color: inherit;
    font: inherit;
    font-size: 12px;
  }
  .add button {
    height: 28px;
    padding: 0 12px;
    border-radius: 14px;
    color: var(--accent);
  }
  .add button:hover:not(:disabled) {
    background: var(--hover);
  }
  .add button:disabled {
    opacity: 0.4;
  }
  .about p {
    margin: 0;
    padding: 6px 16px;
  }
  .about p:first-child {
    padding-top: 14px;
  }
  .about p:last-child {
    padding-bottom: 14px;
  }
  code {
    font-family: "Cascadia Mono", Consolas, monospace;
    font-size: 11.5px;
    background: var(--pressed);
    padding: 1px 5px;
    border-radius: 4px;
  }
  .about-actions button {
    margin-top: 8px;
    padding: 8px 16px;
    border-radius: 6px;
    border: 1px solid var(--line);
    transition: background-color var(--t-fast) var(--ease-out);
  }
  .about-actions button:hover {
    background: var(--hover);
  }
  .danger {
    color: var(--danger);
  }
</style>
