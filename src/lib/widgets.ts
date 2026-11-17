import type { Component } from "svelte";
import AppsTile from "../widgets/apps/AppsTile.svelte";
import ClockTile from "../widgets/clock/ClockTile.svelte";
import ClockCard from "../widgets/clock/ClockCard.svelte";

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
  clock: {
    id: "clock",
    name: "Clock",
    description: "Time with seconds and the date; click for a calendar",
    tile: ClockTile,
    card: ClockCard,
    cardSize: { width: 316, height: 392 },
  },
};
