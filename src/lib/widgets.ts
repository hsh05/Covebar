import type { Component } from "svelte";
import AppsTile from "../widgets/apps/AppsTile.svelte";
import ClockTile from "../widgets/clock/ClockTile.svelte";
import ClockCard from "../widgets/clock/ClockCard.svelte";
import MediaTile from "../widgets/media/MediaTile.svelte";
import MediaCard from "../widgets/media/MediaCard.svelte";
import TimerTile from "../widgets/timer/TimerTile.svelte";
import TimerCard from "../widgets/timer/TimerCard.svelte";
import RecycleTile from "../widgets/recycle/RecycleTile.svelte";
import RecycleCard from "../widgets/recycle/RecycleCard.svelte";
import BrightnessTile from "../widgets/brightness/BrightnessTile.svelte";
import BrightnessCard from "../widgets/brightness/BrightnessCard.svelte";
import MicTile from "../widgets/mic/MicTile.svelte";
import VolumeTile from "../widgets/volume/VolumeTile.svelte";
import VolumeCard from "../widgets/volume/VolumeCard.svelte";
import TrayTile from "../widgets/tray/TrayTile.svelte";
import TemperatureTile from "../widgets/temperature/TemperatureTile.svelte";
import TemperatureCard from "../widgets/temperature/TemperatureCard.svelte";
import ClaudeTile from "../widgets/claude/ClaudeTile.svelte";
import ClaudeCard from "../widgets/claude/ClaudeCard.svelte";
import CurrencyTile from "../widgets/currency/CurrencyTile.svelte";
import CurrencyCard from "../widgets/currency/CurrencyCard.svelte";

export interface WidgetDef {
  id: string;
  name: string;
  description: string;
  tile: Component;
  card?: Component;
  cardSize?: { width: number; height: number };
  /** The card sizes itself to its content; `cardSize.height` is only the first guess. */
  fit?: boolean;
  /** Takes the remaining width of the bar. */
  grow?: boolean;
}

export const WIDGETS: Record<string, WidgetDef> = {
  apps: {
    id: "apps",
    name: "Apps",
    description: "Start button, pinned apps and open windows with live previews",
    tile: AppsTile,
    grow: true,
  },
  media: {
    id: "media",
    name: "Now Playing",
    description: "What's playing in Spotify, your browser or any media app, with controls",
    tile: MediaTile,
    card: MediaCard,
  },
  timer: {
    id: "timer",
    name: "Timer & Stopwatch",
    description: "Countdown with presets that rings when it ends, and a stopwatch with laps",
    tile: TimerTile,
    card: TimerCard,
  },
  recycle: {
    id: "recycle",
    name: "Recycle Bin",
    description: "Item count and size; open or empty it",
    tile: RecycleTile,
    card: RecycleCard,
  },
  currency: {
    id: "currency",
    name: "Currency",
    description: "Live exchange rate with a converter and favourites; works offline from the last rates",
    tile: CurrencyTile,
    card: CurrencyCard,
    cardSize: { width: 320, height: 260 },
    fit: true,
  },
  claude: {
    id: "claude",
    name: "Claude Usage",
    description: "Your Claude plan's session and weekly limits, from the Claude Code sign-in on this PC",
    tile: ClaudeTile,
    card: ClaudeCard,
    fit: true,
  },
  temperature: {
    id: "temperature",
    name: "Temperature",
    description: "CPU and GPU temperature and load (temperatures need LibreHardwareMonitor running)",
    tile: TemperatureTile,
    card: TemperatureCard,
    cardSize: { width: 300, height: 260 },
    fit: true,
  },
  brightness: {
    id: "brightness",
    name: "Brightness",
    description: "Screen brightness for laptop panels and DDC/CI monitors; scroll to change",
    tile: BrightnessTile,
    card: BrightnessCard,
  },
  mic: {
    id: "mic",
    name: "Microphone",
    description: "Click to mute or unmute your microphone; scroll for input level",
    tile: MicTile,
  },
  volume: {
    id: "volume",
    name: "Volume",
    description: "Speaker volume and output device; scroll to change, middle-click to mute",
    tile: VolumeTile,
    card: VolumeCard,
  },
  tray: {
    id: "tray",
    name: "Tray Icons",
    description: "Your notification-area apps; click or right-click them as usual",
    tile: TrayTile,
  },
  clock: {
    id: "clock",
    name: "Clock",
    description: "Time with seconds and the date; click for a calendar",
    tile: ClockTile,
    card: ClockCard,
    cardSize: { width: 316, height: 392 },
  },
};
