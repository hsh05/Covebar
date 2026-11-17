// Right-click menus for the widgets on the bar. Each widget lists its own quick actions; the
// dock adds "Hide from the bar" and "Customize widgets" underneath.
import { invoke } from "@tauri-apps/api/core";
import { audio, displays, media, recycle, timers, timersAction } from "./live.svelte";
import type { MenuEntry } from "./menu";
import { store, updateSettings } from "./settings.svelte";
import { timerPhase } from "../widgets/timer/timer";
import { iconUrl } from "./icons";

const settingsPage = (page: string) => () => invoke("launch_app", { target: `ms-settings:${page}` });

function duration(sec: number) {
  if (sec % 3600 === 0) return `${sec / 3600} h`;
  if (sec % 60 === 0) return `${sec / 60} min`;
  return `${sec} s`;
}

/** `openCard` is the widget's left-click action, for items that need its card. */
export async function widgetMenu(id: string, openCard: () => void): Promise<MenuEntry[]> {
  const s = store.s;
  switch (id) {
    case "media": {
      const m = media().v;
      if (!m) return [{ label: "Nothing playing", icon: "play", disabled: true, action: () => {} }];
      const cmd = (action: string) => () => invoke("media_command", { action });
      return [
        // Album art when there is some, else the player's icon.
        { header: m.title || m.sourceName, image: m.art ?? (m.sourceIcon ? await iconUrl(m.sourceIcon, 20) : null) },
        { separator: true },
        {
          label: m.playing ? "Pause" : "Play",
          icon: m.playing ? "pause" : "play",
          disabled: !m.canPlayPause,
          action: cmd("playPause"),
        },
        { label: "Next track", icon: "next", disabled: !m.canNext, action: cmd("next") },
        { label: "Previous track", icon: "prev", disabled: !m.canPrev, action: cmd("prev") },
      ];
    }
    case "timer": {
      const t = timers().v.timer;
      const phase = timerPhase(t);
      if (phase === "ringing")
        return [
          { label: "Dismiss", icon: "check", action: () => timersAction("timerDismiss") },
          { label: "Restart", icon: "reset", action: () => timersAction("timerStart", Math.round(t.durationMs / 1000)) },
        ];
      if (phase === "running")
        return [
          { label: "Pause", icon: "pause", action: () => timersAction("timerPause") },
          { label: "Add a minute", icon: "plus", action: () => timersAction("timerAdd", 60) },
          { label: "Cancel timer", icon: "close", danger: true, action: () => timersAction("timerReset") },
        ];
      if (phase === "paused")
        return [
          { label: "Resume", icon: "play", action: () => timersAction("timerResume") },
          { label: "Cancel timer", icon: "close", danger: true, action: () => timersAction("timerReset") },
        ];
      return (s?.timerPresets ?? []).slice(0, 6).map((sec) => ({
        label: `Start ${duration(sec)}`,
        icon: "timer" as const,
        action: () => timersAction("timerStart", sec),
      }));
    }
    case "stopwatch": {
      const sw = timers().v.stopwatch;
      const running = sw.startedAt != null;
      return [
        running
          ? { label: "Pause", icon: "pause", action: () => timersAction("swPause") }
          : { label: sw.elapsedMs ? "Resume" : "Start", icon: "play", action: () => timersAction("swStart") },
        { label: "Lap", icon: "flag", disabled: !running, action: () => timersAction("swLap") },
        { label: "Reset", icon: "reset", disabled: !running && sw.elapsedMs === 0, action: () => timersAction("swReset") },
      ];
    }
    case "recycle": {
      const empty = recycle().v.items === 0;
      return [
        { label: "Open Recycle Bin", icon: "open", action: () => invoke("open_recycle") },
        // Goes through the card, which asks before deleting anything.
        { label: "Empty Recycle Bin…", icon: "trash", danger: true, disabled: empty, action: openCard },
      ];
    }
    case "brightness": {
      const list = displays().v;
      const now = list[0]?.brightness;
      const set = (v: number) => () => list.forEach((d) => invoke("set_brightness", { id: d.id, value: v }));
      const levels: MenuEntry[] = [100, 75, 50, 25].map((v) => ({
        label: `${v}%`,
        icon: now === v ? "check" : "sun",
        disabled: !list.length,
        action: set(v),
      }));
      return [...levels, { separator: true }, { label: "Display settings", icon: "settings", action: settingsPage("display") }];
    }
    case "mic": {
      const mic = audio().v.input;
      return [
        {
          label: mic?.muted ? "Unmute microphone" : "Mute microphone",
          icon: mic?.muted ? "mic" : "micOff",
          hint: s?.hotkeys.micMute || undefined,
          disabled: !mic,
          action: () => mic && invoke("set_mute", { which: "input", muted: !mic.muted }),
        },
        { label: "Sound settings", icon: "settings", action: settingsPage("sound") },
      ];
    }
    case "volume": {
      const a = audio().v;
      const out = a.output;
      const devices: MenuEntry[] =
        a.outputs.length > 1
          ? [
              { separator: true },
              ...a.outputs.map((o) => ({
                label: o.name,
                icon: o.id === out?.id ? ("check" as const) : undefined,
                action: () => invoke("set_default_output", { id: o.id }),
              })),
            ]
          : [];
      return [
        {
          label: out?.muted ? "Unmute" : "Mute",
          icon: out?.muted ? "speaker" : "mute",
          hint: s?.hotkeys.speakerMute || undefined,
          disabled: !out,
          action: () => out && invoke("set_mute", { which: "output", muted: !out.muted }),
        },
        ...devices,
        { separator: true },
        { label: "Volume mixer", icon: "sliders", action: settingsPage("apps-volume") },
        { label: "Sound settings", icon: "settings", action: settingsPage("sound") },
      ];
    }
    case "clock":
      return [
        {
          label: "24-hour time",
          icon: s?.clock24h ? "check" : undefined,
          action: () => updateSettings({ clock24h: !s?.clock24h }),
        },
        {
          label: "Show seconds",
          icon: s?.clockSeconds ? "check" : undefined,
          action: () => updateSettings({ clockSeconds: !s?.clockSeconds }),
        },
        { separator: true },
        { label: "Date & time settings", icon: "clock", action: settingsPage("dateandtime") },
      ];
    default:
      return [];
  }
}
