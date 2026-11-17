// Backend state that widgets show, shared by every component in a window. Each store
// fetches once and then follows the backend's change events.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AudioState,
  BinInfo,
  ClaudeUsage,
  CurrencyState,
  DisplayInfo,
  HotkeyStatus,
  MediaInfo,
  Temps,
  TimersState,
  TrayState,
} from "./types";

function live<T>(getter: string, event: string, initial: T) {
  const box = $state<{ v: T; ready: boolean }>({ v: initial, ready: false });
  let started = false;
  return () => {
    if (!started) {
      started = true;
      listen<T>(event, (e) => {
        box.v = e.payload;
        box.ready = true;
      });
      invoke<T>(getter).then((v) => {
        box.v = v;
        box.ready = true;
      });
    }
    return box;
  };
}

export const media = live<MediaInfo | null>("get_media", "media-changed", null);
export const audio = live<AudioState>("get_audio", "audio-changed", { output: null, input: null, outputs: [] });
export const displays = live<DisplayInfo[]>("get_brightness", "brightness-changed", []);
export const recycle = live<BinInfo>("get_recycle", "recycle-changed", { items: 0, bytes: 0 });
export const timers = live<TimersState>("get_timers", "timers-changed", {
  timer: { durationMs: 0, endsAt: null, remainingMs: null, ringing: false },
  stopwatch: { startedAt: null, elapsedMs: 0, laps: [] },
});
export const hotkeyStatus = live<HotkeyStatus>("get_hotkey_status", "hotkeys-status", {
  micMute: "off",
  speakerMute: "off",
});

export const tray = live<TrayState>("get_tray", "tray-changed", { icons: [], hasOverflow: false, unavailable: false });
export const temps = live<Temps>("get_temps", "temps-changed", {
  source: "",
  cpuTemp: null,
  gpuTemp: null,
  cpuLoad: null,
  gpuLoad: null,
  memLoad: null,
  cpuName: null,
  gpuName: null,
});
export const currency = live<CurrencyState>("get_currency", "currency-changed", { rates: null, error: null, loading: false });
export const claude = live<ClaudeUsage>("get_claude", "claude-changed", {
  status: "loading",
  plan: null,
  windows: [],
  fetchedAt: null,
  message: null,
});

export const timersAction = (action: string, value?: number) =>
  invoke<TimersState>("timers_action", { action, value: value ?? null });

/** A clock that ticks every `ms` while `active()` is true. */
export function ticker(ms: number, active: () => boolean) {
  const box = $state({ now: Date.now() });
  $effect(() => {
    if (!active()) return;
    box.now = Date.now();
    const id = setInterval(() => (box.now = Date.now()), ms);
    return () => clearInterval(id);
  });
  return box;
}
