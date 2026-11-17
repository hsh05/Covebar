<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { HotkeyState } from "../lib/types";

  let {
    value,
    fallback,
    status,
    onchange,
  }: { value: string; fallback: string; status: HotkeyState; onchange: (v: string) => void } = $props();

  let recording = $state(false);
  let held = $state<string[]>([]);
  let hint = $state("");

  const MODS: [keyof KeyboardEvent, string][] = [
    ["metaKey", "Win"],
    ["ctrlKey", "Ctrl"],
    ["altKey", "Alt"],
    ["shiftKey", "Shift"],
  ];
  const NAMED: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Tab: "Tab",
    Insert: "Insert",
    Delete: "Delete",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Pause: "Pause",
    Comma: ",",
    Period: ".",
    Minus: "-",
    Equal: "=",
    Semicolon: ";",
    Slash: "/",
    Backquote: "`",
    BracketLeft: "[",
    BracketRight: "]",
    Quote: "'",
    Backslash: "\\",
  };

  function keyName(code: string): string | null {
    if (/^Key[A-Z]$/.test(code)) return code.slice(3);
    if (/^Digit\d$/.test(code)) return code.slice(5);
    if (/^F\d{1,2}$/.test(code)) return code;
    return NAMED[code] ?? null;
  }

  function start() {
    recording = true;
    held = [];
    hint = "";
    invoke("pause_hotkeys", { paused: true });
  }
  function stop() {
    if (!recording) return;
    recording = false;
    held = [];
    invoke("pause_hotkeys", { paused: false });
  }

  function keydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    const mods = MODS.filter(([k]) => e[k]).map(([, n]) => n);
    held = mods;
    if (["Meta", "Control", "Alt", "Shift", "OS"].includes(e.key)) return;
    if (!mods.length) {
      if (e.code === "Escape") return stop();
      if (e.code === "Backspace" || e.code === "Delete") {
        onchange("");
        return stop();
      }
      hint = "Hold Win, Ctrl, Alt or Shift with a key";
      return;
    }
    const key = keyName(e.code);
    if (!key) {
      hint = "That key can't be used in a shortcut";
      return;
    }
    onchange([...mods, key].join("+"));
    stop();
  }
  function keyup(e: KeyboardEvent) {
    if (recording) held = MODS.filter(([k]) => e[k]).map(([, n]) => n);
  }

  const parts = $derived(recording ? held : value ? value.split("+") : []);
  const STATUS: Record<HotkeyState, string> = {
    ok: "Active",
    off: "Off",
    invalid: "Not a valid shortcut",
    taken: "In use by another app; pick another",
  };
</script>

<svelte:window onkeydown={keydown} onkeyup={keyup} />

<div class="rec">
  <button class="keys" class:recording onclick={() => (recording ? stop() : start())} onblur={stop}>
    {#if parts.length}
      {#each parts as p, i (i)}<kbd>{p}</kbd>{/each}
    {:else}
      <span class="placeholder">{recording ? "Press a shortcut…" : "None"}</span>
    {/if}
  </button>
  {#if value !== fallback}
    <button class="reset" onclick={() => onchange(fallback)} title="Back to {fallback}">Reset</button>
  {/if}
</div>
<div class="status {recording ? '' : status}">
  {recording ? hint || "Esc cancels, Backspace turns it off" : STATUS[status]}
</div>

<style>
  .rec {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .keys {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 150px;
    height: 32px;
    padding: 0 8px;
    border-radius: 7px;
    border: 1px solid var(--line);
    background: var(--pressed);
    transition:
      border-color var(--t-fast) var(--ease-out),
      box-shadow var(--t-fast) var(--ease-out);
  }
  .keys:hover {
    background: var(--hover);
  }
  .keys.recording {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  kbd {
    font: 600 11.5px var(--font);
    padding: 2px 7px;
    border-radius: 5px;
    background: var(--raised);
    border: 1px solid var(--line);
    border-bottom-width: 2px;
  }
  .placeholder {
    color: var(--text-faint);
  }
  .reset {
    font-size: 12px;
    color: var(--accent);
    padding: 4px 6px;
    border-radius: 5px;
  }
  .reset:hover {
    background: var(--hover);
  }
  .status {
    margin-top: 4px;
    font-size: 11.5px;
    color: var(--text-dim);
    text-align: right;
  }
  .status.ok {
    color: var(--accent);
  }
  .status.taken,
  .status.invalid {
    color: var(--danger);
  }
</style>
