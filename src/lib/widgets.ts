import type { Component } from "svelte";
import AppsTile from "../widgets/apps/AppsTile.svelte";
import ClockTile from "../widgets/clock/ClockTile.svelte";
import ClockCard from "../widgets/clock/ClockCard.svelte";
import MediaTile from "../widgets/media/MediaTile.svelte";
import MediaCard from "../widgets/media/MediaCard.svelte";
import TimerTile from "../widgets/timer/TimerTile.svelte";
import TimerCard from "../widgets/timer/TimerCard.svelte";
import StopwatchTile from "../widgets/stopwatch/StopwatchTile.svelte";
import StopwatchCard from "../widgets/stopwatch/StopwatchCard.svelte";
import RecycleTile from "../widgets/recycle/RecycleTile.svelte";
import RecycleCard from "../widgets/recycle/RecycleCard.svelte";
import BrightnessTile from "../widgets/brightness/BrightnessTile.svelte";
import BrightnessCard from "../widgets/brightness/BrightnessCard.svelte";
import MicTile from "../widgets/mic/MicTile.svelte";
import VolumeTile from "../widgets/volume/VolumeTile.svelte";
import VolumeCard from "../widgets/volume/VolumeCard.svelte";

export interface WidgetDef {
  id: string;
  name: string;
  description: string;
  tile: Component;
  card?: Component;
  cardSize?: { width: number; height: number };
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
    name: "Timer",
    description: "Countdown with presets; rings and notifies you when it ends",
    tile: TimerTile,
    card: TimerCard,
  },
  stopwatch: {
    id: "stopwatch",
    name: "Stopwatch",
    description: "Stopwatch with laps that are kept across restarts",
    tile: StopwatchTile,
    card: StopwatchCard,
  },
  recycle: {
    id: "recycle",
    name: "Recycle Bin",
    description: "Item count and size; open or empty it",
    tile: RecycleTile,
    card: RecycleCard,
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
  clock: {
    id: "clock",
    name: "Clock",
    description: "Time with seconds and the date; click for a calendar",
    tile: ClockTile,
    card: ClockCard,
    cardSize: { width: 316, height: 392 },
  },
};
