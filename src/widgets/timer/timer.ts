import type { TimerState } from "../../lib/types";

export type TimerPhase = "idle" | "running" | "paused" | "ringing";

export function timerPhase(t: TimerState): TimerPhase {
  if (t.ringing) return "ringing";
  if (t.endsAt != null) return "running";
  if (t.remainingMs != null && t.durationMs > 0) return "paused";
  return "idle";
}

/** Milliseconds left at `now`. */
export function timerLeft(t: TimerState, now: number) {
  if (t.endsAt != null) return Math.max(0, t.endsAt - now);
  return t.remainingMs ?? 0;
}

/** Fraction of the countdown still left, 1 -> 0. */
export function timerFraction(t: TimerState, now: number) {
  return t.durationMs > 0 ? Math.min(1, timerLeft(t, now) / t.durationMs) : 0;
}

export const TIMER_CARD = { width: 300, height: 356 };
