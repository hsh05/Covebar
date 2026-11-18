import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type MenuIcon =
  | "download"
  | "settings" | "swap" | "activity" | "power" | "open" | "plus" | "close"
  | "play" | "pause" | "next" | "prev" | "check" | "reset" | "timer" | "clock" | "flag" | "trash" | "sun" | "mic" | "micOff" | "speaker" | "mute" | "sliders" | "hide" | "pin" | "unpin" | "shield" | "minimize" | "restore" | "folder";

/** What callers pass to `showMenu`. */
export type MenuEntry =
  | { header: string; image?: string | null }
  | { separator: true }
  | { label: string; icon?: MenuIcon; hint?: string; danger?: boolean; disabled?: boolean; action: () => void };

/** What the menu window renders (actions stay here in the dock, keyed by id). */
export type MenuRow =
  | { kind: "header"; label: string; image?: string | null }
  | { kind: "sep" }
  | { kind: "item"; id: number; label: string; icon?: MenuIcon; hint?: string; danger?: boolean; disabled?: boolean };

export const MENU = { width: 248, pad: 5, item: 32, sep: 9, header: 38 };

let session = 0;
let actions = new Map<number, () => void>();

listen<{ session: number; id: number }>("menu-pick", (e) => {
  if (e.payload.session !== session) return;
  const run = actions.get(e.payload.id);
  actions = new Map();
  run?.();
});

/** Open Covebar's own context menu at the cursor. */
export async function showMenu(entries: MenuEntry[]) {
  const next = new Map<number, () => void>();
  const rows: MenuRow[] = entries.map((e, i) => {
    if ("header" in e) return { kind: "header", label: e.header, image: e.image };
    if ("separator" in e) return { kind: "sep" };
    next.set(i, e.action);
    const { action: _, ...rest } = e;
    return { kind: "item", id: i, ...rest };
  });
  const height =
    2 +
    MENU.pad * 2 +
    rows.reduce((h, r) => h + (r.kind === "sep" ? MENU.sep : r.kind === "header" ? MENU.header : MENU.item), 0);
  actions = next;
  session = await invoke<number>("open_menu", { items: rows, width: MENU.width, height });
}
