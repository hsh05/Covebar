import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Settings } from "./types";

export const store = $state<{ s: Settings | null }>({ s: null });

export async function initSettings() {
  store.s = await invoke<Settings>("get_settings");
  await listen<Settings>("settings-changed", (e) => {
    store.s = e.payload;
  });
}

export async function updateSettings(patch: Partial<Settings>) {
  if (!store.s) return;
  const next = { ...$state.snapshot(store.s), ...patch } as Settings;
  store.s = next;
  try {
    store.s = await invoke<Settings>("set_settings", { settings: next });
  } catch (err) {
    console.error("Saving settings failed", err);
    store.s = await invoke<Settings>("get_settings");
    throw err;
  }
}
